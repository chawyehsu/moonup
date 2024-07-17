use clap::Parser;

/// Show installed and currently active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let toolchains = crate::toolchain::installed_toolchains()?;

    println!("Moonup home: {}\n", crate::moonup_home().display());
    if toolchains.is_empty() {
        println!("No toolchains installed");
    } else {
        println!("Installed toolchains:");
        for toolchain in toolchains {
            let tag = match (toolchain.default, toolchain.latest) {
                (true, true) => Some(format!(" (latest, {})", console::style("default").blue())),
                (true, false) => Some(format!(" ({})", console::style("default").blue())),
                (false, true) => Some(" (latest)".to_string()),
                (false, false) => None,
            };

            println!("  {}{}", toolchain.version, tag.unwrap_or_default());
        }
    }

    Ok(())
}
