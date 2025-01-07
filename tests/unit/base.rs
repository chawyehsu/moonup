use insta::assert_snapshot;

use crate::util;

#[test]
fn test_moonup_home() {
    util::apply_common_filters!();

    let home = dirs::home_dir().unwrap().display().to_string();

    temp_env::with_var_unset("MOONUP_HOME", || {
        let mut moonup_home = moonup::moonup_home().display().to_string();
        moonup_home = moonup_home.replace(&home, "~");
        assert_snapshot!(moonup_home, @r"~/.moonup");
    });
}

#[test]
fn test_moonup_home_env() {
    util::apply_common_filters!();

    #[cfg(target_os = "windows")]
    let custom_moonup_home = r"D:\moonup";
    #[cfg(not(target_os = "windows"))]
    let custom_moonup_home = "/opt/moonup";

    temp_env::with_var("MOONUP_HOME", Some(custom_moonup_home), || {
        let moonup_home = moonup::moonup_home().display().to_string();
        #[cfg(target_os = "windows")]
        assert_snapshot!(moonup_home, @"D:/moonup");
        #[cfg(not(target_os = "windows"))]
        assert_snapshot!(moonup_home, @"/opt/moonup");
    });
}

#[test]
fn test_moon_home() {
    util::apply_common_filters!();

    let home = dirs::home_dir().unwrap().display().to_string();

    temp_env::with_var_unset("MOON_HOME", || {
        let mut moon_home = moonup::moon_home().display().to_string();
        moon_home = moon_home.replace(&home, "~");
        assert_snapshot!(moon_home, @r"~/.moon");
    });
}

#[test]
fn test_moon_home_env() {
    util::apply_common_filters!();

    #[cfg(target_os = "windows")]
    let custom_moon_home = r"X:\moonbit";
    #[cfg(not(target_os = "windows"))]
    let custom_moon_home = "/opt/moonbit";

    temp_env::with_var("MOON_HOME", Some(custom_moon_home), || {
        let moon_home = moonup::moon_home().display().to_string();
        #[cfg(target_os = "windows")]
        assert_snapshot!(moon_home, @"X:/moonbit");
        #[cfg(not(target_os = "windows"))]
        assert_snapshot!(moon_home, @"/opt/moonbit");
    });
}
