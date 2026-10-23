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
        if body.contains("usage_not_included") {
            return Self::UsageLimit("your ChatGPT plan does not include this".into());
        }
        let usage_limit = body.contains("usage_limit_reached");
        match status {
            401 | 403 => Self::Auth { status, message },
            429 if usage_limit => Self::UsageLimit(usage_limit_message(&body)),
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

/// "resets in 2 h 5 min" from the ChatGPT backend's `resets_at` (unix seconds), when present.
fn usage_limit_message(body: &str) -> String {
    let resets_at = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["resets_at"].as_u64());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match resets_at {
        Some(t) if t > now => {
            let mins = (t - now).div_ceil(60);
            let (h, m) = (mins / 60, mins % 60);
            let when = if h > 0 { format!("{h} h {m} min") } else { format!("{m} min") };
            format!("ChatGPT plan limit reached; resets in {when}")
        }
        _ => "ChatGPT plan limit reached; try again later".into(),
    }
}
