use clap::Parser;

use crate::toolchain::{installed_toolchains, resolve, InstalledToolchain};

/// Show installed and active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let installed = installed_toolchains()?;

    println!("Moonup home: {}\n", crate::moonup_home().display());
    if installed.is_empty() {
        println!("No toolchains installed");
    } else {
        let default = resolve::detect_default_version();

        println!("Installed toolchains:");
        for toolchain in installed {
            let is_default = match default {
                Some(ref d) => {
                    if toolchain.latest {
                        d == "latest"
                    } else {
                        d == &toolchain.version
                    }
                }
                None => false,
            };

            let tag = match (is_default, toolchain.latest) {
                (true, true) => Some(format!(" (latest, {})", console::style("default").blue())),
                (true, false) => Some(format!(" ({})", console::style("default").blue())),
                (false, true) => Some(" (latest)".to_string()),
                (false, false) => None,
            };

            println!("  {}{}", toolchain.version, tag.unwrap_or_default());
        }

        let active = resolve::detect_active_toolchain();
        let t = InstalledToolchain::from_path(&active)?;
        println!("\nActive toolchain: {}", console::style(t.version).green());
    }

    Ok(())
}
