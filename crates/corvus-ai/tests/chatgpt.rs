//! ChatGPT sign-in against a mock auth server: browser and device flows, refresh, and the
//! Responses backend with ChatGPT auth.

use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use corvus_ai::chatgpt::{ChatGptAuth, Endpoints, Login, Tokens, SECRET_ID};
use corvus_ai::providers::responses::Responses;
use corvus_ai::{AiError, AskEvent, AskRequest, Assistant, ChatMessage};
use corvus_core::{MemoryStore, SecretStore};
use serde_json::{json, Value};
use wiremock::matchers::{body_partial_json, body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn jwt(claims: Value) -> String {
    format!("e30.{}.sig", URL_SAFE_NO_PAD.encode(claims.to_string()))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn token_response(access_exp: u64) -> Value {
    json!({
        "id_token": jwt(json!({
            "email": "tuan@example.com",
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_1", "chatgpt_plan_type": "plus" }
        })),
        "access_token": jwt(json!({ "exp": access_exp })),
        "refresh_token": "refresh-1",
    })
}

fn endpoints(server: &MockServer) -> Endpoints {
    Endpoints { auth_base: server.uri(), backend_base: format!("{}/codex", server.uri()), callback_ports: vec![0] }
}

#[tokio::test]
async fn browser_sign_in_end_to_end() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=the-code"))
        .and(body_string_contains("code_verifier="))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_response(now() + 3600)))
        .mount(&server)
        .await;

    let login = Login::new(endpoints(&server));
    let seen_url = Arc::new(Mutex::new(String::new()));
    let seen = seen_url.clone();
    let browser_page = Arc::new(Mutex::new(String::new()));
    let page = browser_page.clone();
    let tokens = login
        .browser(move |url| {
            *seen.lock().unwrap() = url.to_string();
            // Play the browser: follow the redirect with the code and the same state.
            let u = reqwest::Url::parse(url).unwrap();
            let q = |k: &str| u.query_pairs().find(|(n, _)| n == k).unwrap().1.into_owned();
            let callback = format!("{}?code=the-code&state={}", q("redirect_uri"), q("state"));
            tokio::spawn(async move {
                let body = reqwest::get(callback).await.unwrap().text().await.unwrap();
                *page.lock().unwrap() = body;
            });
        })
        .await
        .unwrap();

    assert_eq!(tokens.email.as_deref(), Some("tuan@example.com"));
    assert_eq!(tokens.account_id.as_deref(), Some("acct_1"));
    assert_eq!(tokens.plan.as_deref(), Some("plus"));
    let url = seen_url.lock().unwrap().clone();
    for expected in ["client_id=app_EMoamEEZ73f0CkXaXp7hrann", "code_challenge_method=S256", "codex_cli_simplified_flow=true", "scope=openid+profile+email+offline_access"] {
        assert!(url.contains(expected), "authorize URL lacks {expected}: {url}");
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(browser_page.lock().unwrap().contains("Signed in to Corvus as tuan@example.com"));
}

#[tokio::test]
async fn browser_sign_in_rejects_wrong_state() {
    let server = MockServer::start().await;
    let login = Login::new(endpoints(&server));
    let result = login
        .browser(|url| {
            let u = reqwest::Url::parse(url).unwrap();
            let redirect = u.query_pairs().find(|(n, _)| n == "redirect_uri").unwrap().1.into_owned();
            tokio::spawn(async move {
                let _ = reqwest::get(format!("{redirect}?code=x&state=forged")).await;
            });
        })
        .await;
    assert!(matches!(result, Err(AiError::NotSignedIn(m)) if m.contains("state mismatch")));
}

#[tokio::test]
async fn device_code_sign_in_polls_until_approved() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/usercode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"device_auth_id":"dev-1","user_code":"ABCD-1234","interval":"1"})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/token"))
        .respond_with(ResponseTemplate::new(403))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/token"))
        .and(body_partial_json(json!({"device_auth_id":"dev-1","user_code":"ABCD-1234"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"authorization_code":"dc","code_verifier":"v","code_challenge":"c"})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("deviceauth%2Fcallback"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_response(now() + 3600)))
        .mount(&server)
        .await;

    let login = Login::new(endpoints(&server));
    let code = login.device_start().await.unwrap();
    assert_eq!(code.user_code, "ABCD-1234");
    assert!(code.verification_url.ends_with("/codex/device"));
    let tokens = login.device_finish(&code).await.unwrap();
    assert_eq!(tokens.account_id.as_deref(), Some("acct_1"));
}

fn sse_ok() -> ResponseTemplate {
    let body = [
        json!({"type":"response.output_text.delta","delta":"Chào bạn"}),
        json!({"type":"response.completed","response":{"usage":{"input_tokens":1,"output_tokens":2}}}),
    ]
    .iter()
    .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap()))
    .collect::<String>();
    ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body)
}

#[tokio::test]
async fn expired_token_is_refreshed_and_saved_before_asking() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_partial_json(json!({"grant_type":"refresh_token","refresh_token":"old-refresh"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token": jwt(json!({"exp": now() + 3600}))})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .and(header("ChatGPT-Account-ID", "acct_1"))
        .and(body_partial_json(json!({"store":false,"stream":true,"instructions":"You are a helpful assistant."})))
        .respond_with(sse_ok())
        .mount(&server)
        .await;

    let stale: Tokens = serde_json::from_value(json!({
        "id_token": jwt(json!({})), "access_token": jwt(json!({"exp": now() - 10})), "refresh_token": "old-refresh",
        "account_id": "acct_1", "email": null, "plan": null, "last_refresh": now() - 100
    }))
    .unwrap();
    let store = Arc::new(MemoryStore::default());
    let auth = ChatGptAuth::new(Login::new(endpoints(&server)), stale, store.clone());
    let r = Responses::new("chatgpt".into(), "ChatGPT".into(), format!("{}/codex", server.uri()), "gpt-5.5".into(), true, Box::new(auth));
    let sink = |_: AskEvent| {};
    let req = AskRequest { system: None, messages: vec![ChatMessage::user("hi")], web_search: false };
    assert_eq!(r.ask(&req, &sink).await.unwrap().message.text, "Chào bạn");

    // The refreshed token was persisted, keeping the refresh token the server didn't resend.
    let saved = Tokens::load(store.as_ref()).unwrap().unwrap();
    assert_eq!(saved.refresh_token, "old-refresh");
    assert_eq!(saved.account_id.as_deref(), Some("acct_1"));
    assert!(store.get(SECRET_ID).unwrap().is_some());
}

#[tokio::test]
async fn unauthorized_response_triggers_one_refresh_and_retry() {
    let server = MockServer::start().await;
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = calls.clone();
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .respond_with(move |r: &Request| {
            let auth = r.headers.get("authorization").unwrap().to_str().unwrap().to_string();
            let n = { let mut c = seen.lock().unwrap(); c.push(auth); c.len() };
            if n == 1 { ResponseTemplate::new(401) } else { sse_ok() }
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_response(now() + 7200)))
        .expect(1)
        .mount(&server)
        .await;
    let fresh: Tokens = serde_json::from_value(json!({
        "id_token": jwt(json!({})), "access_token": "first-token", "refresh_token": "r",
        "account_id": "acct_1", "email": null, "plan": null, "last_refresh": now()
    })).unwrap();
    let auth = ChatGptAuth::new(Login::new(endpoints(&server)), fresh, Arc::new(MemoryStore::default()));
    let r = Responses::new("chatgpt".into(), "ChatGPT".into(), format!("{}/codex", server.uri()), "gpt-5.5".into(), true, Box::new(auth));
    let sink = |_: AskEvent| {};
    let req = AskRequest { system: None, messages: vec![ChatMessage::user("hi")], web_search: false };
    assert!(r.ask(&req, &sink).await.is_ok());
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], "Bearer first-token");
    assert_ne!(calls[1], calls[0]);
}

#[tokio::test]
async fn expired_session_asks_to_sign_in_again() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error":{"code":"refresh_token_expired","message":"expired"}})))
        .mount(&server)
        .await;
    let login = Login::new(endpoints(&server));
    let t: Tokens = serde_json::from_value(json!({"id_token":"a","access_token":"b","refresh_token":"c","account_id":null,"email":null,"plan":null})).unwrap();
    assert!(matches!(login.refresh(&t).await, Err(AiError::NotSignedIn(_))));
}

#[tokio::test]
async fn usage_limit_shows_reset_time_and_models_are_sorted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({"error":{"type":"usage_limit_reached","resets_at": now() + 3 * 3600 + 60}})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/codex/models"))
        .and(query_param("client_version", "0.160.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models":[
            {"slug":"gpt-5.5","display_name":"GPT-5.5","visibility":"list","priority":13},
            {"slug":"secret","display_name":"Hidden","visibility":"hide","priority":0},
            {"slug":"gpt-6.1-sol","display_name":"GPT-6.1-Sol","visibility":"list","priority":1}
        ]})))
        .mount(&server)
        .await;
    let t: Tokens = serde_json::from_value(json!({"id_token":"a","access_token":"b","refresh_token":"c","account_id":"acct","email":null,"plan":null,"last_refresh": now()})).unwrap();
    let login = Login::new(endpoints(&server));
    let models = login.models(&t).await.unwrap();
    assert_eq!(models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(), ["gpt-6.1-sol", "gpt-5.5"]);

    let auth = ChatGptAuth::new(login, t, Arc::new(MemoryStore::default()));
    let r = Responses::new("c".into(), "C".into(), format!("{}/codex", server.uri()), "gpt-5.5".into(), false, Box::new(auth));
    let sink = |_: AskEvent| {};
    let req = AskRequest { system: None, messages: vec![ChatMessage::user("hi")], web_search: false };
    match r.ask(&req, &sink).await {
        Err(AiError::UsageLimit(m)) => assert!(m.contains("resets in 3 h 1 min"), "{m}"),
        other => panic!("unexpected: {other:?}"),
    }
}
