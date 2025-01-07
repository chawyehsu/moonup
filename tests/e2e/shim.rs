use std::process::Command;

use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};

const BIN: &str = "moonup-shim";

fn cli() -> Command {
    Command::new(get_cargo_bin(BIN))
}

#[test]
fn test_run_moonup_shim_directly() {
    assert_cmd_snapshot!(cli());
}
