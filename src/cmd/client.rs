use rustun::client::main;

#[tokio::main]
async fn main() {
    let err = main::run_client().await;
    panic!("{:?}", err);
}
