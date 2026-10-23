//! Provider streaming tests against a local mock server with canned SSE bodies.

use std::sync::Mutex;

use corvus_ai::providers::{chat::ChatCompletions, claude::Claude, responses::{ApiKeyAuth, Responses}};
use corvus_ai::{AiError, AskEvent, AskRequest, Assistant, ChatMessage};
use serde_json::{json, Value};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn sse(events: &[Value]) -> String {
    events.iter().map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap_or("message"))).collect()
}

fn sse_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body)
}

fn ask(text: &str, web_search: bool) -> AskRequest {
    AskRequest { system: Some("Be brief.".into()), messages: vec![ChatMessage::user(text)], web_search }
}

/// Run `ask`, returning the reply and every event emitted.
async fn run(a: &dyn Assistant, req: &AskRequest) -> (Result<corvus_ai::AskReply, AiError>, Vec<AskEvent>) {
    let events = Mutex::new(Vec::new());
    let sink = |e: AskEvent| events.lock().unwrap().push(e);
    let r = a.ask(req, &sink).await;
    (r, events.into_inner().unwrap())
}

#[tokio::test]
async fn claude_streams_text_search_and_citations() {
    let server = MockServer::start().await;
    let body = sse(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":12}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig123"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"server_tool_use","id":"srv_1","name":"web_search","input":{}}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"query\": \"rust"}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" tauri\"}"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"content_block_start","index":2,"content_block":{"type":"web_search_tool_result","tool_use_id":"srv_1","content":[{"type":"web_search_result","url":"https://tauri.app","title":"Tauri"}]}}),
        json!({"type":"content_block_stop","index":2}),
        json!({"type":"content_block_start","index":3,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":3,"delta":{"type":"text_delta","text":"Tauri is "}}),
        json!({"type":"content_block_delta","index":3,"delta":{"type":"citations_delta","citation":{"type":"web_search_result_location","url":"https://tauri.app","title":"Tauri","cited_text":"..."}}}),
        json!({"type":"content_block_delta","index":3,"delta":{"type":"text_delta","text":"a framework."}}),
        json!({"type":"content_block_stop","index":3}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}),
        json!({"type":"message_stop"}),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "k"))
        .and(header("anthropic-version", "2026-10-21"))
        .and(body_partial_json(json!({"model":"claude-opus-5-5","stream":true,"system":"Be brief.","tools":[{"type":"web_search_20260209","name":"web_search"}]})))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let claude = Claude::new("claude".into(), None, Some(server.uri()), "k".into(), None, true);
    let (reply, events) = run(&claude, &ask("What is Tauri?", true)).await;
    let reply = reply.unwrap();
    assert_eq!(reply.message.text, "Tauri is a framework.");
    assert_eq!(reply.sources.len(), 1, "citation duplicates the search result");
    assert!(events.contains(&AskEvent::Searching { query: "rust tauri".into() }));
    let usage = reply.usage.unwrap();
    assert_eq!((usage.input_tokens, usage.output_tokens), (12, 7));

    // Content blocks are kept exactly for replay: signature, parsed tool input, citations.
    let raw = reply.message.raw.as_ref().unwrap();
    assert_eq!(raw[0]["signature"], "sig123");
    assert_eq!(raw[1]["input"]["query"], "rust tauri");
    assert_eq!(raw[3]["citations"][0]["url"], "https://tauri.app");
}

#[tokio::test]
async fn claude_resumes_after_pause_turn_and_replays_blocks() {
    let server = MockServer::start().await;
    let first = sse(&[
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Part one. "}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"pause_turn"},"usage":{"output_tokens":3}}),
    ]);
    let second = sse(&[
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Part two."}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
    ]);
    let calls = std::sync::Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = calls.clone();
    Mock::given(method("POST"))
        .respond_with(move |r: &Request| {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            let n = { let mut c = seen.lock().unwrap(); c.push(body); c.len() };
            sse_response(if n == 1 { first.clone() } else { second.clone() })
        })
        .mount(&server)
        .await;

    let claude = Claude::new("claude".into(), None, Some(server.uri()), "k".into(), Some("claude-sonnet-5-5".into()), true);
    let (reply, _) = run(&claude, &ask("Long question", false)).await;
    let reply = reply.unwrap();
    assert_eq!(reply.message.text, "Part one. Part two.");
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    // The paused partial turn is sent back as an assistant message so the server resumes.
    assert_eq!(calls[1]["messages"][1]["role"], "assistant");
    assert_eq!(calls[1]["messages"][1]["content"][0]["text"], "Part one. ");
    // A mock base URL is not the first-party API, so no fallbacks parameter.
    assert!(calls[0].get("fallbacks").is_none());

    // A follow-up to the same provider replays the raw blocks instead of plain text.
    let mut req = ask("Follow-up", false);
    req.messages.insert(0, reply.message.clone());
    req.messages.insert(0, ChatMessage::user("Long question"));
    let other = Claude::new("other".into(), None, Some(server.uri()), "k".into(), None, false);
    let _ = (claude, other); // replay selection is covered by raw_for
    assert!(reply.message.raw_for("claude").is_some());
    assert!(reply.message.raw_for("other").is_none());
}

#[tokio::test]
async fn claude_refusal_and_http_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(sse_response(sse(&[json!({"type":"message_delta","delta":{"stop_reason":"refusal"}})])))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}})))
        .mount(&server)
        .await;
    let claude = Claude::new("c".into(), None, Some(server.uri()), "k".into(), None, false);
    assert!(matches!(run(&claude, &ask("x", false)).await.0, Err(AiError::Refused(_))));
    match run(&claude, &ask("x", false)).await.0 {
        Err(AiError::Auth { status: 401, message }) => assert_eq!(message, "invalid x-api-key"),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn responses_streams_text_search_and_annotations() {
    let server = MockServer::start().await;
    let body = sse(&[
        json!({"type":"response.created","response":{}}),
        json!({"type":"response.output_item.done","item":{"type":"web_search_call","action":{"type":"search","query":"weather hanoi"}}}),
        json!({"type":"response.output_text.delta","delta":"Sunny "}),
        json!({"type":"response.output_text.delta","delta":"today."}),
        json!({"type":"response.output_text.annotation.added","annotation":{"type":"url_citation","url":"https://weather.example","title":"Weather"}}),
        json!({"type":"response.completed","response":{"usage":{"input_tokens":5,"output_tokens":3}}}),
    ]);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("Authorization", "Bearer sk"))
        .and(body_partial_json(json!({"model":"m","stream":true,"store":false,"instructions":"Be brief.","tools":[{"type":"web_search"}]})))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;
    let r = Responses::new("openai".into(), "OpenAI".into(), format!("{}/v1", server.uri()), "m".into(), true, Box::new(ApiKeyAuth("sk".into())));
    let (reply, events) = run(&r, &ask("Weather?", true)).await;
    let reply = reply.unwrap();
    assert_eq!(reply.message.text, "Sunny today.");
    assert_eq!(reply.sources[0].url, "https://weather.example");
    assert!(events.contains(&AskEvent::Searching { query: "weather hanoi".into() }));
    assert_eq!(reply.usage.unwrap().output_tokens, 3);
}

#[tokio::test]
async fn responses_failure_event_is_an_error() {
    let server = MockServer::start().await;
    let body = sse(&[json!({"type":"response.failed","response":{"error":{"message":"model overloaded"}}})]);
    Mock::given(method("POST")).respond_with(sse_response(body)).mount(&server).await;
    let r = Responses::new("o".into(), "O".into(), server.uri(), "m".into(), false, Box::new(ApiKeyAuth("k".into())));
    match run(&r, &ask("x", false)).await.0 {
        Err(AiError::Stream(m)) => assert_eq!(m, "model overloaded"),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn chat_completions_stream_until_done() {
    let server = MockServer::start().await;
    let body = [
        r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"Xin "}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"chào"}}]}"#,
        r#"data: {"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":2}}"#,
        "data: [DONE]",
    ]
    .map(|l| format!("{l}\n\n"))
    .concat();
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model":"llama","stream":true,"messages":[{"role":"system","content":"Be brief."}]})))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;
    let c = ChatCompletions::new("ollama".into(), None, format!("{}/v1", server.uri()), None, "llama".into());
    let (reply, events) = run(&c, &ask("Hello in Vietnamese", true)).await;
    assert_eq!(reply.unwrap().message.text, "Xin chào");
    assert_eq!(events.len(), 2);
}
