#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("not configured: {0}")]
    Config(String),
    #[error("not signed in: {0}")]
    NotSignedIn(String),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("authentication failed (HTTP {status}): {message}")]
    Auth { status: u16, message: String },
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("usage limit reached: {0}")]
    UsageLimit(String),
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("the model declined to answer{0}")]
    Refused(String),
    #[error("{0}")]
    Stream(String),
    #[error("unexpected response: {0}")]
    Parse(String),
}

impl AiError {
    /// Turn an error response into a readable error, preferring the API's own message.
    pub(crate) async fn from_response(resp: reqwest::Response) -> Self {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let message = api_message(&body).unwrap_or_else(|| {
            let mut b = body.clone();
            b.truncate(300);
            b
        });
        let usage_limit = body.contains("usage_limit_reached") || body.contains("usage_not_included");
        match status {
            401 | 403 => Self::Auth { status, message },
            429 if usage_limit => Self::UsageLimit(message),
            429 => Self::RateLimited(message),
            _ => Self::Http { status, message },
        }
    }
}

/// Extract `error.message` (or `detail`) from the JSON error shapes used by Anthropic and OpenAI.
pub(crate) fn api_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let e = v.get("error").unwrap_or(&v);
    e.get("message")
        .or_else(|| e.get("detail"))
        .and_then(|m| m.as_str())
        .map(str::to_string)
        .or_else(|| v.get("detail").and_then(|d| d.as_str()).map(str::to_string))
}
