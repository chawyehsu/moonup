// The fixture stub: a tiny native executable stub that replaces every binary
// in a shrunken toolchain archive.
//
// The e2e tests only assert on file existence, staging consumption and
// shim re-pour. Succeed when invoked without arguments, signal SIGTERM
// when requested.
//
// Cross-compiled with `zig cc`; see README.md in this directory.
#include <signal.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc > 1 && strcmp(argv[1], "sigterm") == 0) {
#if defined(__unix__) || defined(__APPLE__)
    signal(SIGTERM, SIG_DFL);
    raise(SIGTERM);
#endif
    return 1;
  }

  return argc > 1 ? 1 : 0;
}
