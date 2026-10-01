//! A Nostr relay for the end-to-end tests (spec 024, T078): the same
//! in-process `MockRelay` the integration tests use, run as a process so two
//! application processes can find each other through it without a network.
//! Prints its address (`ws://127.0.0.1:<port>`) as the only line on stdout and
//! runs until it is killed. Built only with `--features e2e`.

#[tokio::main]
async fn main() {
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("the mock relay starts");
    println!("{}", relay.url().await);
    // The relay lives as long as this value does.
    std::future::pending::<()>().await;
    drop(relay);
}
