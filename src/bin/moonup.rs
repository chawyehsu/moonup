#[tokio::main]
pub async fn main() {
    if let Err(err) = moonup::cli::start().await {
        eprintln!("{err:?}");
        std::process::exit(1);
    }
}
