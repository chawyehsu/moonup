# Changelog

## [0.2.0](https://github.com/chawyehsu/moonup/compare/v0.1.6...v0.2.0) (2025-01-07)


### ⚠ BREAKING CHANGES

* Add `MOONUP_DIST_SERVER` support
* new implementation against the new server api ([#50](https://github.com/chawyehsu/moonup/issues/50))

### Features

* **cli/default:** show nightly installs ([80ddac9](https://github.com/chawyehsu/moonup/commit/80ddac9704b66fb3689da741b1ed422dcf3c31b1))
* **cli/pin:** show nightly installs ([80ddac9](https://github.com/chawyehsu/moonup/commit/80ddac9704b66fb3689da741b1ed422dcf3c31b1))
* **cli/show:** show nightly installs ([80ddac9](https://github.com/chawyehsu/moonup/commit/80ddac9704b66fb3689da741b1ed422dcf3c31b1))
* **dev:** add tests ([#59](https://github.com/chawyehsu/moonup/issues/59)) ([92736c5](https://github.com/chawyehsu/moonup/commit/92736c58515029cb96a1b11f797d771ec383f05d))
* new implementation against the new server api ([#50](https://github.com/chawyehsu/moonup/issues/50)) ([8730d35](https://github.com/chawyehsu/moonup/commit/8730d35a2b44f184079ba4702cd21226077420b8))
* **subcommands:** reflect new implementation ([80ddac9](https://github.com/chawyehsu/moonup/commit/80ddac9704b66fb3689da741b1ed422dcf3c31b1))


### Code Refactoring

* Add `MOONUP_DIST_SERVER` support ([f5013e4](https://github.com/chawyehsu/moonup/commit/f5013e4e88719f7bc95d3734acf9112679f0da7a)), closes [#50](https://github.com/chawyehsu/moonup/issues/50)

## [0.1.6](https://github.com/chawyehsu/moonup/compare/v0.1.5...v0.1.6) (2025-01-06)


### Bug Fixes

* filter bin directory, fix regression in b852702 ([977fd51](https://github.com/chawyehsu/moonup/commit/977fd51ae1d273578eb086cde4986b8819ef3d4d))

## [0.1.5](https://github.com/chawyehsu/moonup/compare/v0.1.4...v0.1.5) (2025-01-02)


### Features

* support new toolchain archive layout ([#47](https://github.com/chawyehsu/moonup/issues/47)) ([b852702](https://github.com/chawyehsu/moonup/commit/b8527025c6f6f4dfb1ccf2cb4fc08bbe0584ce6c))

## [0.1.4](https://github.com/chawyehsu/moonup/compare/v0.1.3...v0.1.4) (2024-12-26)


### Features

* **cli:** add aliases for the show command ([e429289](https://github.com/chawyehsu/moonup/commit/e429289d05c545e9435892a9b14f66a86cd8b55e))
* **cli:** set max length for select dialogs ([2de5ad8](https://github.com/chawyehsu/moonup/commit/2de5ad83bb2f70d14eac8fa95f312332db2895f3))


### Bug Fixes

* **cli:** show command should list active toolchain ([ee2fefc](https://github.com/chawyehsu/moonup/commit/ee2fefcbe798b536e80aa3d7587abef93dda4748))
* filter out non executables when creating shims ([ccee680](https://github.com/chawyehsu/moonup/commit/ccee680177046a89e1a11d9f2f259a44d622b71a))
* handle unsuccessful http responses in url_to_reader ([077a715](https://github.com/chawyehsu/moonup/commit/077a7152974448332ddd6c843b410c38daf01ba7))

## [0.1.3](https://github.com/chawyehsu/moonup/compare/v0.1.2...v0.1.3) (2024-10-18)


### Features

* **moonup-shim:** implement `moon upgrade` interception ([#14](https://github.com/chawyehsu/moonup/issues/14)) ([b8f118b](https://github.com/chawyehsu/moonup/commit/b8f118bbb256736f6646ca2693346f9a2e31381a))
* **moonup:** support `MOONUP_TOOLCHAIN_INDEX` configuration ([ece4859](https://github.com/chawyehsu/moonup/commit/ece4859851b8b508af42c61d49e403d126a8c39c))


### Bug Fixes

* **moonup-shim:** fix shim name checking on windows ([6d78280](https://github.com/chawyehsu/moonup/commit/6d7828014741e5c01a78051d662e491463aae8bc))

## [0.1.2](https://github.com/chawyehsu/moonup/compare/v0.1.1...v0.1.2) (2024-09-18)


### Bug Fixes

* **install:** Delete obsolete files before new installation (Close [#15](https://github.com/chawyehsu/moonup/issues/15)) ([6dffcc7](https://github.com/chawyehsu/moonup/commit/6dffcc77fd90ae0802fc813dfd5bcf31b1cf414e))
* **install:** Filter out non exe files in `bin` on Windows ([79ee083](https://github.com/chawyehsu/moonup/commit/79ee0830fd5f40517c523275a82d7ea082c399ba))

## [0.1.1](https://github.com/chawyehsu/moonup/compare/v0.1.0...v0.1.1) (2024-08-01)


### Features

* Support `moonup install` using pinned version [#12](https://github.com/chawyehsu/moonup/issues/12) ([857cadc](https://github.com/chawyehsu/moonup/commit/857cadc6bb222adee8a3f87a9e40eddfcb1a33aa))


### Bug Fixes

* Print help when `moonup pin` missing toolchain version ([1f3dd90](https://github.com/chawyehsu/moonup/commit/1f3dd90c54bc211456c2ef344532af04ca4084ad))

## [0.1.0](https://github.com/chawyehsu/moonup/compare/v0.1.0-rc.2...v0.1.0) (2024-07-25)


### Features

* Add `moonup run` command ([5904fd7](https://github.com/chawyehsu/moonup/commit/5904fd7769e029a1b629430ea901bcb1b24edcb2))

## [0.1.0-rc.2](https://github.com/chawyehsu/moonup/compare/v0.1.0-rc.1...v0.1.0-rc.2) (2024-07-23)


### Features

* Add download progress reporting ([1349d0c](https://github.com/chawyehsu/moonup/commit/1349d0c9f6707ae96948d633ef3c8638f9ae9133))
* Link `lib` directory ([#8](https://github.com/chawyehsu/moonup/issues/8)) ([372d4a7](https://github.com/chawyehsu/moonup/commit/372d4a7859948f474338aa4014302828a0d95aec))


### Bug Fixes

* Check before renaming executable ([76d7085](https://github.com/chawyehsu/moonup/commit/76d708529a0d880e0c1ca2628601dc81309d80ed))
* Ensure command `moonup default` working directory exist ([c894aa2](https://github.com/chawyehsu/moonup/commit/c894aa2dc82c42db7f6d98c163cf0f382d32461f))
* Improve installed toolchains lookup ([7cfa3ca](https://github.com/chawyehsu/moonup/commit/7cfa3cab019edb62d9c51ee381c7394ee7915f06))
* Only set env `MOON_CORE_OVERRIDE` for `moon` command ([656694b](https://github.com/chawyehsu/moonup/commit/656694bba9fe7d66dc35eb61ce336cebd7c2c73c))

## [0.1.0-rc.1](https://github.com/chawyehsu/moonup/compare/v0.1.0-beta.2...v0.1.0-rc.1) (2024-07-22)


### Features

* Add `moonup completions` command ([eed3cd9](https://github.com/chawyehsu/moonup/commit/eed3cd996e0b1ececcd285ab717da583d795f627))
* Add `moonup update` command ([ed04db0](https://github.com/chawyehsu/moonup/commit/ed04db097f4d581d70ccf4154cf6f811393e7b01))
* Add subcommand aliases ([9b254bd](https://github.com/chawyehsu/moonup/commit/9b254bd7997beb8950ef3a9b810c01b366f41f6b))
* Support listing installable toolchain versions ([2d60a73](https://github.com/chawyehsu/moonup/commit/2d60a73e42c879bf580bdf7c6f058da1f43275ad))


### Bug Fixes

* Fix crate keywords typo ([3cba98c](https://github.com/chawyehsu/moonup/commit/3cba98ceff498e1e1e9acd375f6d65744a164475))
* Handle error of installing invalid version of toolchain ([9c6302c](https://github.com/chawyehsu/moonup/commit/9c6302c67bc107121618a6515fadfd136a86a19c))
* Remove unnecessary argument derive ([93189f4](https://github.com/chawyehsu/moonup/commit/93189f4b3c718964456ee4e17f7374811e34d178))
* Write `version` file after executable extraction ([7172bea](https://github.com/chawyehsu/moonup/commit/7172bea219b1c86f4ddd1e05b8b285b0bfb2e690))

## [0.1.0-beta.2](https://github.com/chawyehsu/moonup/compare/v0.1.0-beat.1...v0.1.0-beta.2) (2024-07-17)


### Features

* Add interactive prompt to `default` and `pin` commands ([e8c6e1c](https://github.com/chawyehsu/moonup/commit/e8c6e1c34486ab698fe0f7fcc442278589f455de))
* Add main cli's metadata ([4b68260](https://github.com/chawyehsu/moonup/commit/4b68260dbcd7e5460ad47be29a988565f95be848))
* Install active toolchain if not already installed ([fcf957d](https://github.com/chawyehsu/moonup/commit/fcf957ddcd7256f5dfb8273bd135234d5a45eafc))


### Bug Fixes

* Ensure shim pouring even if shim is locked and running ([c1f446b](https://github.com/chawyehsu/moonup/commit/c1f446b7e8ce20718bb2d9307517782e68841e13))
* Fix typo of release version number ([21284b0](https://github.com/chawyehsu/moonup/commit/21284b070c24fc3f75539f156962566732c1bf2b))
* Set env `MOON_CORE_OVERRIDE` for the shim ([192199f](https://github.com/chawyehsu/moonup/commit/192199f77a02f46d857e646d931fc228dc5322c9))

## [0.1.0-beat.1](https://github.com/chawyehsu/moonup/compare/v0.1.0-alpha.1...v0.1.0-beat.1) (2024-07-17)


### Features

* Add `moonup default` command ([101fb2a](https://github.com/chawyehsu/moonup/commit/101fb2a4fe033b946a786f6d2000ec2fbeb2b3fe))
* Add `moonup show` command ([26c336b](https://github.com/chawyehsu/moonup/commit/26c336baaa2a85ec412d7096ea62474644db5b58))
* Add `moonup which` command ([724d177](https://github.com/chawyehsu/moonup/commit/724d1779b3a1d48f475df9f07835a7f6f5d788b9))
* cache actual version number for `latest` install ([0fd4ff3](https://github.com/chawyehsu/moonup/commit/0fd4ff39c665d9cbbd921177d7a5a27f79e8cee7))
* Display toolchain file path after pinning ([63e1468](https://github.com/chawyehsu/moonup/commit/63e1468d21c2dfdd2f426fcc8184169a9d93e6e6))

## 0.1.0-alpha.1 (2024-07-16)


### Features

* add pin command ([751fe1f](https://github.com/chawyehsu/moonup/commit/751fe1fc920e45a7f60cfe2c8edfc7c5f5503efd))
* first shim implementation ([ba43aad](https://github.com/chawyehsu/moonup/commit/ba43aad7d040402a25b57a8de27c79d5e82b8e46))
* implement package downloading ([a9a9571](https://github.com/chawyehsu/moonup/commit/a9a95711b8d838c968495d48ef015a9bb7f7addb))
* implement post installation ([e4a14b8](https://github.com/chawyehsu/moonup/commit/e4a14b872756a3055649fc977e964794fbe2c6af))
* install latest toolchain to folder named `latest` ([9bd6193](https://github.com/chawyehsu/moonup/commit/9bd61931d235ff256f50ce7a6d4198d34e1dda6e))


### Bug Fixes

* ensure moon home dir exist ([5ec49f2](https://github.com/chawyehsu/moonup/commit/5ec49f255ae569d7394665d60183d2ce64f31fa9))
* fix ci artifact packaging ([1657d96](https://github.com/chawyehsu/moonup/commit/1657d96687fa6e8b5a509860bac26f8b0ebd8d66))
* fix ci prod build on linux ([23c999c](https://github.com/chawyehsu/moonup/commit/23c999c112d579b75ceeb84de0a486a30a6dfb17))
* match correct extension ([4f27a7e](https://github.com/chawyehsu/moonup/commit/4f27a7e703304dbf6aaac604a6748c821d692765))
* set executable permissions on unix ([ff44b0b](https://github.com/chawyehsu/moonup/commit/ff44b0b28dda71ef46e73d6611797f2c84163044))
