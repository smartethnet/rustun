use rustun::server::main;

#[tokio::main]
async fn main() {
    main::run_server().await;
}