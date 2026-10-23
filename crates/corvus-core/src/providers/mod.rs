pub mod deepl;
pub mod google;
pub mod openai;

use std::time::Duration;

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("corvus/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("TLS backend available")
}
