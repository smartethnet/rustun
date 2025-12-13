use rustun::server::runner;

#[tokio::main]
async fn main() {
    runner::run_server().await;
}