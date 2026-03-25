use rustun::client::main;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    main::run_client().await
}
