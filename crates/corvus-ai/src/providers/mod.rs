pub mod chat;
pub mod claude;
pub mod responses;

use std::time::Duration;

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("corvus/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        // Streams can be long; only stalls between chunks should time out.
        .read_timeout(Duration::from_secs(120))
        .build()
        .expect("TLS backend available")
}
