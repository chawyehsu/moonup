#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
    Build Moonup and package release artifacts.

.DESCRIPTION
    Builds Moonup with pixi for the requested platform and packages the
    resulting binaries into the release archives expected by the release
    workflow. This script is used by .github/workflows/build.yml but can be
    run locally on any supported platform.

    Extraction of the conda package produced by `pixi build` is delegated to
    `rattler-build package extract`; the binaries are then repacked from the
    staging tree.

    The build temporarily mutates `pixi.toml` (cargo profile override, macOS
    SDK path); the original manifest is always restored afterwards.

.PARAMETER Platform
    The build target to produce. One of: linux, linux-aarch64, macos, windows.
    Defaults to the platform matching the current host.

.PARAMETER BuildType
    The artifact type: "release" or "nightly". Nightly builds use the "ci"
    cargo profile. Defaults to "release".

.PARAMETER OutputDir
    Directory the final packaged artifacts are written to. Defaults to "out".
    Intermediate files (conda packages, staging trees) are written to
    ".pixi/output" instead.
#>
[CmdletBinding()]
param(
    [ValidateSet('linux', 'linux-aarch64', 'macos', 'windows')]
    [string] $Platform,

    [ValidateSet('release', 'nightly')]
    [string] $BuildType = 'release',

    [string] $OutputDir = 'out'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The script reads/mutates pixi.toml relative to the repo root, and
# [System.IO.Path]::GetFullPath does not track PowerShell's location, so
# anchor to the script directory and resolve the output dir once.
Set-Location -LiteralPath $PSScriptRoot
$OutputDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDir)
# Intermediate build outputs (conda packages, staging trees) live under .pixi
# so that $OutputDir contains only uploadable release artifacts.
$PackageDir = Join-Path $PSScriptRoot '.pixi/output'

# Point kache's store and runtime directory at its per-user cache directory
# and keep CARGO_HOME on the host. The pixi-build sandbox receives these
# through `[package.build] secrets`.
. (Join-Path $PSScriptRoot 'scripts/env.ps1')

function Invoke-Checked {
    param(
        [Parameter(Mandatory)][string] $Command,
        [string[]] $Arguments = @()
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Command $($Arguments -join ' ')"
    }
}

function Invoke-Pixi {
    param([string[]] $Arguments = @())

    Invoke-Checked -Command 'pixi' -Arguments $Arguments
}

function Invoke-RattlerBuild {
    param([string[]] $Arguments = @())

    Invoke-Pixi -Arguments (@('run', '-e', 'build', 'rattler-build') + $Arguments)
}

function Get-DefaultPlatform {
    if ($IsWindows) { return 'windows' }
    if ($IsMacOS) { return 'macos' }
    if ($IsLinux -and [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') {
        return 'linux-aarch64'
    }
    if ($IsLinux) { return 'linux' }
    throw 'Unsupported host platform.'
}

function Get-TargetPlatform {
    # The conda target platform of the build.
    param([Parameter(Mandatory)][string] $PlatformValue)

    switch ($PlatformValue) {
        'linux' { return 'linux-64' }
        'linux-aarch64' { return 'linux-aarch64' }
        'macos' { return 'osx-arm64' }
        'windows' { return 'win-64' }
    }
}

function Get-TargetTriple {
    # The Rust target triple, used to name the release archives.
    param([Parameter(Mandatory)][string] $PlatformValue)

    switch ($PlatformValue) {
        'linux' { return 'x86_64-unknown-linux-gnu' }
        'linux-aarch64' { return 'aarch64-unknown-linux-gnu' }
        'macos' { return 'aarch64-apple-darwin' }
        'windows' { return 'x86_64-pc-windows-msvc' }
    }
}

function Update-PixiManifest {
    param(
        [Parameter(Mandatory)][scriptblock] $Transform
    )

    $content = Get-Content -Path 'pixi.toml' -Raw
    Set-Content -Path 'pixi.toml' -Value (& $Transform $content) -NoNewline
}

function Set-CargoProfile {
    # The default profile is "release", which is what pixi.toml already
    # specifies. Non-release builds swap it for the "ci" profile.
    if ($BuildType -eq 'release') {
        return
    }

    Update-PixiManifest { param($content) $content -replace '"--profile", "release"', '"--profile", "ci"' }
}

function Set-MacOSSDKPath {
    param([Parameter(Mandatory)][string] $SdkRoot)

    Update-PixiManifest { param($content) $content.Replace('__MACOS_SDK_ROOT__', $SdkRoot) }
}

function Install-NativeMacOSSDK {
    # Native builds use the SDK bundled with the host Xcode toolchain.
    if (-not $IsMacOS) {
        throw "The 'macos' platform requires a macOS host."
    }

    $sdkRoot = @(& xcrun --show-sdk-path 2>$null) | Select-Object -First 1
    if ($LASTEXITCODE -ne 0 -or -not $sdkRoot) {
        throw 'Unable to determine the macOS SDK path via xcrun.'
    }

    Set-MacOSSDKPath -SdkRoot $sdkRoot.Trim()
}

function Build-CondaPackage {
    param([Parameter(Mandatory)][string] $TargetPlatform)

    Invoke-Pixi -Arguments @('run', '-e', 'build', 'package', '--target-platform', $TargetPlatform)

    $package = Get-ChildItem -Path . -Filter 'moonup*.conda' -File | Select-Object -First 1
    if (-not $package) {
        throw "No conda package produced for $TargetPlatform."
    }

    # Normalize the package name so callers get a predictable path.
    $dest = Join-Path $PackageDir "moonup-$TargetPlatform.conda"
    Move-Item -Path $package.FullName -Destination $dest -Force
    return $dest
}

function Expand-CondaPackage {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $Destination
    )

    if (Test-Path -Path $Destination) {
        Remove-Item -Path $Destination -Recurse -Force
    }
    Invoke-RattlerBuild -Arguments @('package', 'extract', $PackageFile, '--dest', $Destination)
}

function Invoke-LinuxStrip {
    # Only the Linux targets need an explicit strip; the other platforms are
    # handled by the cargo profile itself.
    param([Parameter(Mandatory)][string] $BinDir)

    if ($Platform -ne 'linux' -and $Platform -ne 'linux-aarch64') {
        return
    }

    Invoke-Checked -Command 'strip' -Arguments @(
        (Join-Path $BinDir 'moonup'), (Join-Path $BinDir 'moonup-shim')
    )
}

function Write-LinuxAbiReport {
    param([Parameter(Mandatory)][string] $Path)

    # Only the Linux targets produce ELF binaries.
    if ($Platform -ne 'linux' -and $Platform -ne 'linux-aarch64') {
        return
    }

    & ldd -v $Path

    $glibc = & objdump -T $Path | Select-String 'GLIBC' |
    ForEach-Object { if ($_.Line -match 'GLIBC_([.0-9]+)') { $Matches[1] } } |
    Sort-Object -Unique
    Write-Host ($glibc -join "`n")
}

function New-UnixArchive {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $Target
    )

    $artifactName = "moonup-$Target.tar.gz"
    $staging = Join-Path $PackageDir "staging-$Target"
    Expand-CondaPackage -PackageFile $PackageFile -Destination $staging

    $binDir = Join-Path $staging 'bin'
    Invoke-LinuxStrip -BinDir $binDir

    $archive = Join-Path $OutputDir $artifactName
    Invoke-Checked -Command 'tar' -Arguments @(
        '-czf', $archive,
        '-C', $binDir, 'moonup', 'moonup-shim',
        '-C', $PSScriptRoot, 'LICENSE', 'README.md'
    )

    Write-LinuxAbiReport -Path (Join-Path $binDir 'moonup')
    Remove-Item -Path $staging -Recurse -Force
    return $artifactName
}

function New-WindowsArchive {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $Target
    )

    $artifactName = "moonup-$Target.zip"
    $staging = Join-Path $PackageDir "staging-$Target"
    Expand-CondaPackage -PackageFile $PackageFile -Destination $staging

    $binDir = Join-Path $staging 'bin'
    $archive = Join-Path $OutputDir $artifactName
    Compress-Archive -Force -DestinationPath $archive -Path @(
        (Join-Path $binDir 'moonup.exe'),
        (Join-Path $binDir 'moonup-shim.exe'),
        (Join-Path $PSScriptRoot 'LICENSE'),
        (Join-Path $PSScriptRoot 'README.md')
    )

    Remove-Item -Path $staging -Recurse -Force
    return $artifactName
}

function Get-WindowsBuildDirectory {
    param([Parameter(Mandatory)][string] $RepoRoot)

    # The short-path junction works around MAX_PATH limits on GitHub Actions
    # Windows runners. Only use it in CI: local checkouts build in place, so a
    # stale C:\w from another checkout can never be picked up.
    if ($env:GITHUB_ACTIONS -ne 'true') {
        return $RepoRoot
    }

    $junction = 'C:\w'
    $repoRoot = (Get-Item -LiteralPath $RepoRoot).FullName.TrimEnd('\', '/')

    if (Test-Path -LiteralPath $junction) {
        $item = Get-Item -LiteralPath $junction -Force
        $targetProp = $item.PSObject.Properties['Target']
        $target = if ($targetProp) { @($targetProp.Value) | Select-Object -First 1 } else { $item.LinkTarget }
        if (-not $target) {
            throw "The path '$junction' already exists but is not a junction to '$repoRoot'. Remove it and retry."
        }
        $target = [System.IO.Path]::GetFullPath($target).TrimEnd('\', '/')
        if ($target -ine $repoRoot) {
            throw "The existing junction '$junction' points to '$target', not '$repoRoot'. Remove it and retry."
        }
    } else {
        New-Item -ItemType Junction -Path $junction -Target $repoRoot | Out-Null
    }

    return $junction
}

# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

$Platform = if ($Platform) { $Platform } else { Get-DefaultPlatform }

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null

# The build mutates pixi.toml (cargo profile override, macOS SDK path); always
# restore the original manifest, even on failure, so local runs leave no diff.
$manifest = Get-Content -Path 'pixi.toml' -Raw
try {
    Set-CargoProfile

    switch ($Platform) {
        'linux' {
            $package = Build-CondaPackage -TargetPlatform 'linux-64'
            New-UnixArchive -PackageFile $package -Target 'x86_64-unknown-linux-gnu' | Out-Null
        }
        'linux-aarch64' {
            $package = Build-CondaPackage -TargetPlatform 'linux-aarch64'
            New-UnixArchive -PackageFile $package -Target 'aarch64-unknown-linux-gnu' | Out-Null
        }
        'macos' {
            Install-NativeMacOSSDK

            $package = Build-CondaPackage -TargetPlatform 'osx-arm64'
            New-UnixArchive -PackageFile $package -Target 'aarch64-apple-darwin' | Out-Null
        }
        'windows' {
            $workingDir = Get-WindowsBuildDirectory -RepoRoot $PWD.Path
            Push-Location -LiteralPath $workingDir
            try {
                $package = Build-CondaPackage -TargetPlatform 'win-64'
                New-WindowsArchive -PackageFile $package -Target 'x86_64-pc-windows-msvc' | Out-Null
            } finally {
                Pop-Location
            }
        }
    }
} finally {
    Set-Content -Path 'pixi.toml' -Value $manifest -NoNewline
}

Write-Host "Build and packaging completed for $Platform ($BuildType)."
