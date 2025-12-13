use rustun::client::runner;

#[tokio::main]
async fn main() {
    let _ = runner::run_client();
}
