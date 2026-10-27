//! Plays spoken audio from the backend. The webview can't be trusted with it: WebKitGTK hands
//! media to GStreamer, and without an MP3 decoder installed it hangs the whole window.

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, Player};
use tokio::sync::oneshot;

type Request = (Vec<u8>, oneshot::Sender<Result<(), String>>);

/// The output stream isn't `Send`, so one thread owns it and takes clips over a channel.
static PLAYER: LazyLock<Mutex<Sender<Request>>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("audio".into())
        .spawn(move || run(rx))
        .expect("spawn audio thread");
    Mutex::new(tx)
});

/// Plays an MP3 clip, cutting off whatever was playing. Returns once playback has started.
pub async fn play(mp3: Vec<u8>) -> Result<(), String> {
    let (done, started) = oneshot::channel();
    PLAYER.lock().unwrap().send((mp3, done)).map_err(|_| "audio playback stopped working".to_string())?;
    started.await.map_err(|_| "audio playback stopped working".to_string())?
}

fn run(rx: Receiver<Request>) {
    let mut next = rx.recv().ok();
    while let Some((mp3, started)) = next.take() {
        let session = start(mp3);
        let _ = started.send(session.as_ref().map(|_| ()).map_err(Clone::clone));
        // Keep the device open until the clip ends or another arrives, then release it.
        if let Ok((_device, player)) = session {
            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(request) => break next = Some(request),
                    Err(RecvTimeoutError::Timeout) if player.empty() => break,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        if next.is_none() {
            next = rx.recv().ok();
        }
    }
}

fn start(mp3: Vec<u8>) -> Result<(rodio::MixerDeviceSink, Player), String> {
    let source = Decoder::new_mp3(Cursor::new(mp3)).map_err(|e| format!("could not decode audio: {e}"))?;
    let mut device = DeviceSinkBuilder::open_default_sink().map_err(|e| format!("no audio output: {e}"))?;
    device.log_on_drop(false);
    let player = Player::connect_new(device.mixer());
    player.append(source);
    Ok((device, player))
}
