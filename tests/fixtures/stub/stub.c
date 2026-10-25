// The fixture stub: a tiny native executable stub that replaces every binary
// in a shrunken toolchain archive.
//
// The e2e tests only assert on file existence, staging consumption and
// shim re-pour. Succeed when invoked without arguments; reject any command.
//
// Cross-compiled with `zig cc`; see README.md in this directory.
int main(int argc, char **argv) {
  (void)argv;
  return argc > 1 ? 1 : 0;
}
