use clap::{CommandFactory, Parser};
use clap_complete::Shell;

use crate::cli::Cli;

/// Generate shell completions
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// The shell type
    #[clap(long, short)]
    shell: Shell,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let mut buf = vec![];
    clap_complete::generate(
        args.shell,
        &mut Cli::command(),
        env!("CARGO_PKG_NAME"),
        &mut buf,
    );
    println!(
        "{}",
        String::from_utf8(buf).expect("clap_complete did not generate valid shell script")
    );
    Ok(())
}
