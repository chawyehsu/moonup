use std::process::Command;

use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};

const BIN: &str = "moonup";

fn cli() -> Command {
    Command::new(get_cargo_bin(BIN))
}

#[test]
fn test_help() {
    assert_cmd_snapshot!(cli().arg("--help"));
    assert_cmd_snapshot!("default", cli().arg("default").arg("--help"));
    assert_cmd_snapshot!("install", cli().arg("install").arg("--help"));
    assert_cmd_snapshot!("pin", cli().arg("pin").arg("--help"));
    assert_cmd_snapshot!("run", cli().arg("run").arg("--help"));
    assert_cmd_snapshot!("show", cli().arg("show").arg("--help"));
    assert_cmd_snapshot!("update", cli().arg("update").arg("--help"));
    assert_cmd_snapshot!("which", cli().arg("which").arg("--help"));
}

#[test]
fn test_cmd_alias() {
    assert_cmd_snapshot!("install_alias", cli().arg("i").arg("--help"));
    assert_cmd_snapshot!("show_alias", cli().arg("list").arg("--help"));
    assert_cmd_snapshot!("show_alias", cli().arg("ls").arg("--help"));
    assert_cmd_snapshot!("update_alias", cli().arg("u").arg("--help"));
}
