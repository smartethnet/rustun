use rustun::client::runner;

#[tokio::main]
async fn main() {
    let err = runner::run_client().await;
    panic!("{:?}", err);
}
