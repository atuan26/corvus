//! "Sign in with ChatGPT": the OAuth flow the Codex CLI uses, so a ChatGPT plan can power Ask.
//! Protocol details follow the openai/codex source (codex-rs/login). OpenAI has not published
//! this as a public API; it may change, so failures surface clearly instead of silently.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use corvus_core::SecretStore;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::error::AiError;
use crate::providers::responses::ResponsesAuth;

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Models newer than the client version sent are hidden by the backend.
const CLIENT_VERSION: &str = "0.160.0";
const ORIGINATOR: &str = "corvus";
const SCOPE: &str = "openid profile email offline_access";
/// Keychain entry for the token bundle.
pub const SECRET_ID: &str = "chatgpt-oauth";
const REFRESH_BEFORE_EXPIRY_SECS: u64 = 5 * 60;
const REFRESH_AFTER_SECS: u64 = 8 * 24 * 60 * 60;
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Service locations; overridable for tests.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth_base: String,
    pub backend_base: String,
    /// Codex's redirect allow-list covers these ports; the first free one is used.
    pub callback_ports: Vec<u16>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth_base: "https://auth.openai.com".into(),
            backend_base: "https://chatgpt.com/backend-api/codex".into(),
            callback_ports: vec![1455, 1457],
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn random_b64(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Decode a JWT's payload without verifying it; tokens come straight from OpenAI over TLS and
/// are only used to read display/account claims.
fn jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tokens {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
    #[serde(default)]
    pub last_refresh: u64,
}

impl Tokens {
    /// Build from a token endpoint response; refresh responses may omit fields, which are kept.
    fn from_response(v: &Value, previous: Option<&Tokens>) -> Result<Self, AiError> {
        let field = |name: &str| {
            v[name].as_str().map(str::to_string).or_else(|| previous.map(|p| match name {
                "id_token" => p.id_token.clone(),
                "access_token" => p.access_token.clone(),
                _ => p.refresh_token.clone(),
            }))
        };
        let missing = |n: &str| AiError::Parse(format!("token response has no {n}"));
        let mut t = Tokens {
            id_token: field("id_token").ok_or_else(|| missing("id_token"))?,
            access_token: field("access_token").ok_or_else(|| missing("access_token"))?,
            refresh_token: field("refresh_token").ok_or_else(|| missing("refresh_token"))?,
            account_id: None,
            email: None,
            plan: None,
            last_refresh: now(),
        };
        let id = jwt_claims(&t.id_token).unwrap_or(Value::Null);
        let access = jwt_claims(&t.access_token).unwrap_or(Value::Null);
        let auth_claim = |c: &Value, k: &str| c["https://api.openai.com/auth"][k].as_str().map(str::to_string);
        t.account_id = auth_claim(&id, "chatgpt_account_id").or_else(|| auth_claim(&access, "chatgpt_account_id"));
        t.plan = auth_claim(&id, "chatgpt_plan_type").or_else(|| auth_claim(&access, "chatgpt_plan_type"));
        t.email = id["email"]
            .as_str()
            .or(id["https://api.openai.com/profile"]["email"].as_str())
            .map(str::to_string);
        // A refresh may return only an access token; keep what we already knew.
        if let Some(p) = previous {
            t.account_id = t.account_id.or_else(|| p.account_id.clone());
            t.plan = t.plan.or_else(|| p.plan.clone());
            t.email = t.email.or_else(|| p.email.clone());
        }
        Ok(t)
    }

    fn needs_refresh(&self) -> bool {
        let expiring = jwt_claims(&self.access_token)
            .and_then(|c| c["exp"].as_u64())
            .is_some_and(|exp| exp <= now() + REFRESH_BEFORE_EXPIRY_SECS);
        expiring || now().saturating_sub(self.last_refresh) > REFRESH_AFTER_SECS
    }

    pub fn load(store: &dyn SecretStore) -> Result<Option<Self>, String> {
        match store.get(SECRET_ID)? {
            Some(json) => serde_json::from_str(&json).map(Some).map_err(|e| format!("saved ChatGPT sign-in is unreadable: {e}")),
            None => Ok(None),
        }
    }

    pub fn save(&self, store: &dyn SecretStore) -> Result<(), String> {
        store.set(SECRET_ID, &serde_json::to_string(self).map_err(|e| e.to_string())?)
    }
}

/// Shown to the user while they approve a device-code sign-in on another device.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceCode {
    pub verification_url: String,
    pub user_code: String,
    #[serde(skip)]
    device_auth_id: String,
    #[serde(skip)]
    interval: Duration,
}

#[derive(Clone)]
pub struct Login {
    endpoints: Endpoints,
    client: reqwest::Client,
}

impl Login {
    pub fn new(endpoints: Endpoints) -> Self {
        Self { endpoints, client: crate::providers::http_client() }
    }

    /// Browser sign-in: listen on localhost, hand the authorize URL to `open_url`, wait for the
    /// redirect, then exchange the code. Dropping the future cancels and frees the port.
    pub async fn browser(&self, open_url: impl FnOnce(&str)) -> Result<Tokens, AiError> {
        let (listener, port) = self.bind().await?;
        let redirect_uri = format!("http://localhost:{port}/auth/callback");
        let verifier = random_b64(64);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_b64(32);
        let url = reqwest::Url::parse_with_params(
            &format!("{}/oauth/authorize", self.endpoints.auth_base),
            &[
                ("response_type", "code"),
                ("client_id", CLIENT_ID),
                ("redirect_uri", redirect_uri.as_str()),
                ("scope", SCOPE),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("id_token_add_organizations", "true"),
                ("codex_cli_simplified_flow", "true"),
                ("state", state.as_str()),
                ("originator", ORIGINATOR),
            ],
        )
        .map_err(|e| AiError::Config(e.to_string()))?;
        open_url(url.as_str());

        let wait = async {
            loop {
                let (stream, _) = listener.accept().await.map_err(|e| AiError::Stream(e.to_string()))?;
                if let Some(result) = self.handle_callback(stream, &state, &verifier, &redirect_uri).await {
                    return result;
                }
            }
        };
        tokio::time::timeout(LOGIN_TIMEOUT, wait)
            .await
            .map_err(|_| AiError::NotSignedIn("sign-in timed out".into()))?
    }

    async fn bind(&self) -> Result<(TcpListener, u16), AiError> {
        for &port in &self.endpoints.callback_ports {
            if let Ok(l) = TcpListener::bind(("127.0.0.1", port)).await {
                let port = l.local_addr().map(|a| a.port()).unwrap_or(port);
                return Ok((l, port));
            }
        }
        Err(AiError::Config(format!(
            "ports {:?} are in use (is another sign-in, e.g. `codex login`, running?). Use device-code sign-in instead.",
            self.endpoints.callback_ports
        )))
    }

    /// Serve one HTTP request. `None` means "not the callback, keep waiting".
    async fn handle_callback(
        &self,
        mut stream: TcpStream,
        state: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Option<Result<Tokens, AiError>> {
        let mut buf = vec![0u8; 8192];
        let mut len = 0;
        while len < buf.len() {
            match stream.read(&mut buf[len..]).await {
                Ok(0) | Err(_) => break,
                Ok(n) => len += n,
            }
            if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&buf[..len]);
        let target = request.split_whitespace().nth(1).unwrap_or("/");
        let url = reqwest::Url::parse(&format!("http://localhost{target}")).ok()?;
        if url.path() != "/auth/callback" {
            respond(&mut stream, "404 Not Found", "Not found").await;
            return None;
        }
        let param = |k: &str| url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());

        let result = if param("state").as_deref() != Some(state) {
            Err(AiError::NotSignedIn("sign-in response did not match (state mismatch)".into()))
        } else if let Some(err) = param("error") {
            let desc = param("error_description").unwrap_or_default();
            Err(AiError::NotSignedIn(format!("{err} {desc}").trim().to_string()))
        } else if let Some(code) = param("code") {
            self.exchange(&code, verifier, redirect_uri).await
        } else {
            Err(AiError::NotSignedIn("no authorization code returned".into()))
        };
        let page = match &result {
            Ok(t) => format!("Signed in to Corvus{}. You can close this tab.", t.email.as_ref().map(|e| format!(" as {e}")).unwrap_or_default()),
            Err(e) => format!("Sign-in failed: {e}. Return to Corvus to try again."),
        };
        respond(&mut stream, if result.is_ok() { "200 OK" } else { "400 Bad Request" }, &page).await;
        Some(result)
    }

    async fn exchange(&self, code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens, AiError> {
        let resp = self
            .client
            .post(format!("{}/oauth/token", self.endpoints.auth_base))
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(AiError::from_response(resp).await);
        }
        Tokens::from_response(&resp.json().await?, None)
    }

    /// Start a device-code sign-in, for when the browser can't reach localhost.
    pub async fn device_start(&self) -> Result<DeviceCode, AiError> {
        let resp = self
            .client
            .post(format!("{}/api/accounts/deviceauth/usercode", self.endpoints.auth_base))
            .json(&json!({ "client_id": CLIENT_ID }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(AiError::from_response(resp).await);
        }
        let v: Value = resp.json().await?;
        let interval = v["interval"]
            .as_u64()
            .or_else(|| v["interval"].as_str().and_then(|s| s.trim().parse().ok()))
            .unwrap_or(5)
            .max(1);
        Ok(DeviceCode {
            verification_url: format!("{}/codex/device", self.endpoints.auth_base),
            user_code: v["user_code"].as_str().or(v["usercode"].as_str()).unwrap_or_default().into(),
            device_auth_id: v["device_auth_id"].as_str().unwrap_or_default().into(),
            interval: Duration::from_secs(interval),
        })
    }

    /// Poll until the user approves the device code (or it times out).
    pub async fn device_finish(&self, code: &DeviceCode) -> Result<Tokens, AiError> {
        let deadline = tokio::time::Instant::now() + DEVICE_TIMEOUT;
        loop {
            let resp = self
                .client
                .post(format!("{}/api/accounts/deviceauth/token", self.endpoints.auth_base))
                .json(&json!({ "device_auth_id": code.device_auth_id, "user_code": code.user_code }))
                .send()
                .await?;
            match resp.status().as_u16() {
                // Not approved yet.
                403 | 404 => {}
                s if (200..300).contains(&s) => {
                    let v: Value = resp.json().await?;
                    let redirect = format!("{}/deviceauth/callback", self.endpoints.auth_base);
                    return self
                        .exchange(
                            v["authorization_code"].as_str().unwrap_or_default(),
                            v["code_verifier"].as_str().unwrap_or_default(),
                            &redirect,
                        )
                        .await;
                }
                _ => return Err(AiError::from_response(resp).await),
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(AiError::NotSignedIn("the code expired before it was approved".into()));
            }
            tokio::time::sleep(code.interval).await;
        }
    }

    pub async fn refresh(&self, tokens: &Tokens) -> Result<Tokens, AiError> {
        let resp = self
            .client
            .post(format!("{}/oauth/token", self.endpoints.auth_base))
            .json(&json!({ "grant_type": "refresh_token", "client_id": CLIENT_ID, "refresh_token": tokens.refresh_token }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            let permanent = ["refresh_token_expired", "refresh_token_reused", "refresh_token_invalidated", "invalid_grant"]
                .iter()
                .any(|c| body.contains(c));
            if permanent || status == 401 {
                return Err(AiError::NotSignedIn("the ChatGPT session expired; sign in again".into()));
            }
            return Err(AiError::Http { status, message: crate::error::api_message(&body).unwrap_or(body) });
        }
        Tokens::from_response(&resp.json().await?, Some(tokens))
    }

    /// Return tokens that are good for a request, refreshing (and saving) them if needed.
    pub async fn fresh(&self, tokens: Tokens, store: Arc<dyn SecretStore>) -> Result<Tokens, AiError> {
        if !tokens.needs_refresh() {
            return Ok(tokens);
        }
        let fresh = self.refresh(&tokens).await?;
        let saved = fresh.clone();
        let _ = tokio::task::spawn_blocking(move || saved.save(store.as_ref())).await;
        Ok(fresh)
    }

    /// Models this account may use, best first.
    pub async fn models(&self, tokens: &Tokens) -> Result<Vec<ModelEntry>, AiError> {
        let mut req = self
            .client
            .get(format!("{}/models", self.endpoints.backend_base))
            .query(&[("client_version", CLIENT_VERSION)])
            .bearer_auth(&tokens.access_token)
            .header("originator", ORIGINATOR);
        if let Some(id) = &tokens.account_id {
            req = req.header("ChatGPT-Account-ID", id);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AiError::from_response(resp).await);
        }
        let v: Value = resp.json().await?;
        let mut models: Vec<(i64, ModelEntry)> = v["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["visibility"].as_str() != Some("hide"))
            .filter_map(|m| {
                let slug = m["slug"].as_str()?.to_string();
                let name = m["display_name"].as_str().unwrap_or(&slug).to_string();
                Some((m["priority"].as_i64().unwrap_or(i64::MAX), ModelEntry { slug, name }))
            })
            .collect();
        models.sort_by_key(|(p, _)| *p);
        Ok(models.into_iter().map(|(_, m)| m).collect())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModelEntry {
    pub slug: String,
    pub name: String,
}

async fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Corvus</title>\
         <body style=\"font-family:system-ui;margin:3rem\"><h1>Corvus</h1><p>{}</p></body></html>",
        message.replace('&', "&amp;").replace('<', "&lt;")
    );
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Request authentication for the ChatGPT backend, refreshing tokens as needed and saving
/// refreshed tokens back to the store.
pub struct ChatGptAuth {
    login: Login,
    tokens: tokio::sync::Mutex<Tokens>,
    store: Arc<dyn SecretStore>,
    /// The account's best model, looked up once when no model is configured.
    best_model: tokio::sync::Mutex<Option<String>>,
}

impl ChatGptAuth {
    pub fn new(login: Login, tokens: Tokens, store: Arc<dyn SecretStore>) -> Self {
        Self { login, tokens: tokio::sync::Mutex::new(tokens), store, best_model: tokio::sync::Mutex::new(None) }
    }

    async fn refresh_locked(&self, tokens: &mut Tokens) -> Result<(), AiError> {
        *tokens = self.login.refresh(tokens).await?;
        let (store, saved) = (self.store.clone(), tokens.clone());
        // Keychain access can block; a failed save only means refreshing again next launch.
        let _ = tokio::task::spawn_blocking(move || saved.save(store.as_ref())).await;
        Ok(())
    }
}

#[async_trait]
impl ResponsesAuth for ChatGptAuth {
    fn kind(&self) -> &'static str {
        "chatgpt"
    }

    async fn headers(&self) -> Result<Vec<(String, String)>, AiError> {
        let mut tokens = self.tokens.lock().await;
        if tokens.needs_refresh() {
            self.refresh_locked(&mut tokens).await?;
        }
        let mut h = vec![
            ("Authorization".into(), format!("Bearer {}", tokens.access_token)),
            ("originator".into(), ORIGINATOR.into()),
        ];
        if let Some(id) = &tokens.account_id {
            h.push(("ChatGPT-Account-ID".into(), id.clone()));
        }
        Ok(h)
    }

    /// Which models a ChatGPT plan may use varies by plan and changes over time, so "best
    /// available" asks the backend instead of assuming a model name.
    async fn resolve_model(&self, configured: &str) -> Result<String, AiError> {
        if !configured.is_empty() {
            return Ok(configured.to_string());
        }
        let mut cached = self.best_model.lock().await;
        if let Some(m) = cached.as_ref() {
            return Ok(m.clone());
        }
        let tokens = {
            let mut tokens = self.tokens.lock().await;
            if tokens.needs_refresh() {
                self.refresh_locked(&mut tokens).await?;
            }
            tokens.clone()
        };
        let best = self
            .login
            .models(&tokens)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| AiError::Config("this ChatGPT account has no models available".into()))?
            .slug;
        *cached = Some(best.clone());
        Ok(best)
    }

    async fn on_unauthorized(&self) -> Result<bool, AiError> {
        let mut tokens = self.tokens.lock().await;
        self.refresh_locked(&mut tokens).await?;
        Ok(true)
    }

    fn adjust_body(&self, body: &mut Value) {
        // The backend expects instructions on every request.
        if body.get("instructions").is_none() {
            body["instructions"] = "You are a helpful assistant.".into();
        }
        if let Some(tools) = body["tools"].as_array_mut() {
            for t in tools.iter_mut().filter(|t| t["type"] == "web_search") {
                t["external_web_access"] = true.into();
            }
        }
    }
}

