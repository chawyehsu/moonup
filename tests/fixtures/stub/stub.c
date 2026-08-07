// The fixture stub: a tiny native executable stub that replaces every binary
// in a shrunken toolchain archive.
//
// The e2e tests only assert on file existence, staging consumption and
// shim re-pour, so the stub's only contract is to exec and exit 0 (satisfying
// `post_install`'s `moon -C <lib>/core bundle --all`).
//
// Cross-compiled with `zig cc`; see README.md in this directory.
int main(void) { return 0; }
