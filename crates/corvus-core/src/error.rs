use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("not configured: {0}")]
    Config(String),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    #[error("authentication failed (HTTP {status})")]
    Auth { status: u16 },
    #[error("rate limited (HTTP 429)")]
    RateLimited,
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("unexpected response: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl ProviderError {
    /// Map a non-success HTTP response to an error, keeping a short body excerpt for diagnostics.
    pub(crate) async fn from_response(resp: reqwest::Response) -> Self {
        let status = resp.status().as_u16();
        match status {
            401 | 403 => Self::Auth { status },
            429 => Self::RateLimited,
            _ => {
                let mut body = resp.text().await.unwrap_or_default();
                body.truncate(300);
                Self::Http { status, body }
            }
        }
    }
}
