# Changelog

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
