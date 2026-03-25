use rustun::server::main;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    main::run_server().await
}
