//! Ask: streaming answers from AI assistants, and ChatGPT sign-in.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use corvus_ai::chatgpt::{DeviceCode, Endpoints, Login, ModelEntry, Tokens};
use corvus_ai::{registry, AiError, AskEvent, AskReply, AskRequest};
use corvus_core::{FileStore, KeychainOrFile, SecretStore};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::AppState;

pub struct AskState {
    pub secrets: Arc<KeychainOrFile>,
    endpoints: Endpoints,
    running: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    sign_in: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    device_code: Mutex<Option<DeviceCode>>,
}

impl AskState {
    pub fn new() -> Self {
        let path = FileStore::default_path().unwrap_or_else(|| std::env::temp_dir().join("corvus-secrets.json"));
        Self {
            secrets: Arc::new(KeychainOrFile { file: FileStore::new(path) }),
            endpoints: Endpoints::default(),
            running: Mutex::new(HashMap::new()),
            sign_in: Mutex::new(None),
            device_code: Mutex::new(None),
        }
    }

    fn store(&self) -> Arc<dyn SecretStore> {
        self.secrets.clone()
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AssistantStatus {
    id: String,
    name: String,
    kind: corvus_core::AssistantKind,
    model: Option<String>,
    web_search: bool,
    /// Why it can't be used yet, e.g. "Not signed in to ChatGPT".
    problem: Option<String>,
}

#[tauri::command]
pub async fn list_assistants(app: AppHandle) -> Result<Vec<AssistantStatus>, String> {
    let configs = app.state::<AppState>().config().assistants;
    let (store, endpoints) = {
        let s = app.state::<AskState>();
        (s.store(), s.endpoints.clone())
    };
    blocking(move || {
        configs
            .into_iter()
            .filter(|c| c.enabled)
            .map(|c| {
                let built = registry::build(&c, store.clone(), &endpoints);
                let (model, web_search) = match &built {
                    Ok(a) => (Some(a.info().model), a.info().web_search),
                    Err(_) => (c.model.clone(), c.web_search),
                };
                AssistantStatus {
                    name: c.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| registry::default_name(c.kind).into()),
                    id: c.id,
                    kind: c.kind,
                    model,
                    web_search,
                    problem: built.err(),
                }
            })
            .collect()
    })
    .await
}

/// Messages on an answer's stream channel.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamMessage {
    Event { event: AskEvent },
    Done { reply: AskReply },
    Error { message: String, needs_sign_in: bool },
}

fn error_message(e: &AiError) -> StreamMessage {
    StreamMessage::Error {
        message: e.to_string(),
        needs_sign_in: matches!(e, AiError::NotSignedIn(_) | AiError::Auth { .. }),
    }
}

/// Start streaming an answer; results arrive on `channel`. `request_id` lets the UI cancel.
#[tauri::command]
pub async fn ask_start(
    app: AppHandle,
    request_id: String,
    assistant_id: String,
    request: AskRequest,
    channel: Channel<StreamMessage>,
) -> Result<(), String> {
    let config = app
        .state::<AppState>()
        .config()
        .assistants
        .into_iter()
        .find(|a| a.id == assistant_id)
        .ok_or_else(|| format!("No assistant \"{assistant_id}\""))?;
    let (store, endpoints) = {
        let s = app.state::<AskState>();
        (s.store(), s.endpoints.clone())
    };
    let assistant = blocking(move || registry::build(&config, store, &endpoints)).await??;

    let app2 = app.clone();
    let id = request_id.clone();
    let task = tauri::async_runtime::spawn(async move {
        let sink_channel = channel.clone();
        let sink = move |event: AskEvent| {
            let _ = sink_channel.send(StreamMessage::Event { event });
        };
        let message = match assistant.ask(&request, &sink).await {
            Ok(reply) => StreamMessage::Done { reply },
            Err(e) => error_message(&e),
        };
        let _ = channel.send(message);
        app2.state::<AskState>().running.lock().unwrap().remove(&id);
    });
    app.state::<AskState>().running.lock().unwrap().insert(request_id, task);
    Ok(())
}

#[tauri::command]
pub fn ask_cancel(state: State<'_, AskState>, request_id: String) {
    if let Some(task) = state.running.lock().unwrap().remove(&request_id) {
        task.abort();
    }
}

#[derive(Serialize)]
pub struct ChatGptStatus {
    signed_in: bool,
    email: Option<String>,
    plan: Option<String>,
    /// "keychain" or "file": where the sign-in is kept.
    storage: &'static str,
}

#[tauri::command]
pub async fn chatgpt_status(state: State<'_, AskState>) -> Result<ChatGptStatus, String> {
    let secrets = state.secrets.clone();
    blocking(move || {
        let tokens = Tokens::load(secrets.as_ref())?;
        Ok(ChatGptStatus {
            signed_in: tokens.is_some(),
            email: tokens.as_ref().and_then(|t| t.email.clone()),
            plan: tokens.and_then(|t| t.plan),
            storage: secrets.location(),
        })
    })
    .await?
}

async fn finish_sign_in(app: &AppHandle, result: Result<Tokens, AiError>) -> Result<ChatGptStatus, String> {
    let tokens = result.map_err(|e| e.to_string())?;
    let secrets = app.state::<AskState>().secrets.clone();
    blocking(move || tokens.save(secrets.as_ref())).await??;
    let _ = app.emit("settings-changed", ());
    chatgpt_status(app.state::<AskState>()).await
}

/// Run a sign-in future so `chatgpt_cancel_sign_in` can stop it.
async fn cancellable<T: Send + 'static>(
    app: &AppHandle,
    fut: impl std::future::Future<Output = T> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task = tauri::async_runtime::spawn(async move {
        let _ = tx.send(fut.await);
    });
    if let Some(old) = app.state::<AskState>().sign_in.lock().unwrap().replace(task) {
        old.abort();
    }
    rx.await.map_err(|_| "Sign-in cancelled".to_string())
}

/// Sign in through the browser. Resolves when the browser redirect arrives.
#[tauri::command]
pub async fn chatgpt_sign_in(app: AppHandle) -> Result<ChatGptStatus, String> {
    let login = Login::new(app.state::<AskState>().endpoints.clone());
    let opener = app.clone();
    let result = cancellable(&app, async move {
        login
            .browser(move |url| {
                if let Err(e) = opener.opener().open_url(url, None::<&str>) {
                    tracing::warn!("could not open the browser: {e}");
                }
            })
            .await
    })
    .await?;
    finish_sign_in(&app, result).await
}

/// Start device-code sign-in: the UI shows the code and URL, then calls `chatgpt_device_finish`.
#[tauri::command]
pub async fn chatgpt_device_start(state: State<'_, AskState>) -> Result<DeviceCode, String> {
    let code = Login::new(state.endpoints.clone()).device_start().await.map_err(|e| e.to_string())?;
    *state.device_code.lock().unwrap() = Some(code.clone());
    Ok(code)
}

#[tauri::command]
pub async fn chatgpt_device_finish(app: AppHandle) -> Result<ChatGptStatus, String> {
    let code = app.state::<AskState>().device_code.lock().unwrap().clone().ok_or("Start device sign-in first")?;
    let login = Login::new(app.state::<AskState>().endpoints.clone());
    let result = cancellable(&app, async move { login.device_finish(&code).await }).await?;
    finish_sign_in(&app, result).await
}

#[tauri::command]
pub fn chatgpt_cancel_sign_in(state: State<'_, AskState>) {
    if let Some(task) = state.sign_in.lock().unwrap().take() {
        task.abort();
    }
}

#[tauri::command]
pub async fn chatgpt_sign_out(app: AppHandle) -> Result<(), String> {
    let secrets = app.state::<AskState>().secrets.clone();
    blocking(move || secrets.delete(corvus_ai::chatgpt::SECRET_ID)).await??;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn chatgpt_models(state: State<'_, AskState>) -> Result<Vec<ModelEntry>, String> {
    let store = state.store();
    let s2 = store.clone();
    let tokens = blocking(move || Tokens::load(s2.as_ref())).await??.ok_or("Not signed in to ChatGPT")?;
    let login = Login::new(state.endpoints.clone());
    let tokens = login.fresh(tokens, store).await.map_err(|e| e.to_string())?;
    login.models(&tokens).await.map_err(|e| e.to_string())
}

/// Store (or remove) an assistant's API key: keychain, or the private file without one.
#[tauri::command]
pub async fn set_assistant_key(app: AppHandle, assistant_id: String, key: Option<String>) -> Result<(), String> {
    let secrets = app.state::<AskState>().secrets.clone();
    let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    blocking(move || match key {
        Some(k) => secrets.set(&assistant_id, &k),
        None => secrets.delete(&assistant_id),
    })
    .await??;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

/// Open http(s) links from answers in the user's browser.
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
