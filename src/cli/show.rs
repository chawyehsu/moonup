use clap::Parser;
use miette::IntoDiagnostic;

/// Show installed and currently active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let toolchains_dir = crate::moonup_home().join("toolchains");

    let toolchains = toolchains_dir
        .read_dir()
        .into_diagnostic()?
        .filter_map(std::io::Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let version = path.file_name().map(|n| {
                let n = n.to_ascii_lowercase();
                if n == "latest" {
                    match std::fs::read_to_string(path.join("version"))
                        .ok()
                        .map(|s| s.trim().to_string())
                    {
                        Some(v) => format!("{} (latest)", v),
                        None => "latest".to_string(),
                    }
                } else {
                    n.to_string_lossy().to_string()
                }
            });
            version
        })
        .collect::<Vec<_>>();

    println!("Moonup home: {}\n", crate::moonup_home().display());
    if toolchains.is_empty() {
        println!("No toolchains installed");
    } else {
        println!("Installed toolchains:");
        for toolchain in toolchains {
            println!("  {}", toolchain);
        }
    }

    Ok(())
}
