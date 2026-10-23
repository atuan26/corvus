//! Provider wire-format tests against a local mock server.

use corvus_core::providers::{deepl::DeepL, google::GoogleFree, openai::OpenAiCompatible};
use corvus_core::{ProviderError, TranslateRequest, Translator};
use serde_json::json;
use wiremock::matchers::{body_partial_json, body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req() -> TranslateRequest {
    TranslateRequest::new("Hello", "auto", "vi")
}

#[tokio::test]
async fn google_free() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/translate_a/single"))
        .and(query_param("client", "gtx"))
        .and(body_string_contains("tl=vi"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([[["Xin chào", "Hello"]], null, "en"])))
        .mount(&server)
        .await;
    let t = GoogleFree::new("google", Some(server.uri())).translate(&req()).await.unwrap();
    assert_eq!(t.text, "Xin chào");
    assert_eq!(t.detected_source.as_deref(), Some("en"));
}

#[tokio::test]
async fn deepl() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/translate"))
        .and(header("Authorization", "DeepL-Auth-Key k:fx"))
        .and(body_partial_json(json!({ "target_lang": "VI" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({ "translations": [{ "detected_source_language": "EN", "text": "Xin chào" }] }),
        ))
        .mount(&server)
        .await;
    let t = DeepL::new("deepl", "k:fx".into(), Some(server.uri())).translate(&req()).await.unwrap();
    assert_eq!(t.text, "Xin chào");
    assert_eq!(t.detected_source.as_deref(), Some("en"));
}

#[tokio::test]
async fn deepl_bad_key_is_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(403)).mount(&server).await;
    let err = DeepL::new("deepl", "bad".into(), Some(server.uri())).translate(&req()).await.unwrap_err();
    assert!(matches!(err, ProviderError::Auth { status: 403 }));
}

#[tokio::test]
async fn openai_compatible() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer sk"))
        .and(body_partial_json(json!({ "model": "m" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({ "choices": [{ "message": { "role": "assistant", "content": " Xin chào\n" } }] }),
        ))
        .mount(&server)
        .await;
    let p = OpenAiCompatible::new("ai", None, format!("{}/v1/", server.uri()), Some("sk".into()), "m".into());
    assert_eq!(p.translate(&req()).await.unwrap().text, "Xin chào");
}

#[tokio::test]
async fn rate_limit_is_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(429)).mount(&server).await;
    let err = GoogleFree::new("google", Some(server.uri())).translate(&req()).await.unwrap_err();
    assert!(matches!(err, ProviderError::RateLimited));
}
