#[tokio::main]
async fn main() -> std::process::ExitCode {
    match moonup::cli::start().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            if moonup::status::should_report(&err) {
                eprintln!("{err:?}");
            }
            moonup::status::exit_code_of(&err)
        }
    }
}
