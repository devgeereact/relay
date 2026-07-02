// Relay — Tauri entry point.
//
// Boots the window with the Svelte frontend and opens the local database.
// Real pipeline commands (audio, STT, detection, routing, channels) get wired
// here as each module below is built out — don't front-load functionality.

mod audio;
mod channels;
mod db;
mod detection;
mod router;
mod stt;

use audio::AudioEngine;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::Emitter;

/// The open SQLite connection, guarded for shared access across commands.
/// rusqlite's Connection is not Sync, so a Mutex is required in Tauri state.
struct Db(Mutex<Connection>);

/// The currently-running audio capture engine, if any.
#[derive(Default)]
struct Audio(Mutex<Option<AudioEngine>>);

/// Per-chunk metadata pushed to the frontend on `audio://chunk`. Deliberately
/// does NOT carry the raw samples — the console only needs level + voicing to
/// drive the meter; STT (Phase 4) consumes the samples through a separate path.
#[derive(Clone, Serialize)]
struct ChunkEvent {
    timestamp_ms: u64,
    sample_rate: u32,
    rms: f32,
    is_voice: bool,
    samples: usize,
}

fn main() {
    // Open the on-device DB at startup. Failing here is intentional and loud:
    // a broken data layer must surface before a service, never mid-sermon.
    let conn = db::open().expect("failed to open Relay database");

    tauri::Builder::default()
        .manage(Db(Mutex::new(conn)))
        .manage(Audio::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            lookup_verse,
            data_health,
            list_audio_devices,
            start_capture,
            stop_capture
        ])
        .run(tauri::generate_context!())
        .expect("error while running Relay");
}

// Bridge liveness probe — the frontend calls this on mount to tell whether the
// Rust core is attached (see App.svelte). Cheap, no side effects.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Relay is running. Hello, {name}.")
}

/// Look up a verse by canonical reference for the operator console / manual
/// override. Errors are returned as strings for the frontend to surface —
/// no panics on a live path.
#[tauri::command]
fn lookup_verse(
    db: tauri::State<'_, Db>,
    book: String,
    chapter: i64,
    verse: i64,
) -> Result<Option<db::VerseRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::lookup_verse(&conn, &book, chapter, verse).map_err(|e| e.to_string())
}

/// Number of verses currently seeded — surfaced in Settings as a data-layer
/// health indicator.
#[tauri::command]
fn data_health(db: tauri::State<'_, Db>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::verse_count(&conn).map_err(|e| e.to_string())
}

/// List available audio input devices for the Settings picker.
#[tauri::command]
fn list_audio_devices() -> Vec<audio::DeviceInfo> {
    audio::list_input_devices()
}

/// Start capturing from `device` (default input when None). Each produced chunk
/// is emitted to the frontend as `audio://chunk` (metadata only). Replaces any
/// capture already running.
#[tauri::command]
fn start_capture(
    app: tauri::AppHandle,
    audio: tauri::State<'_, Audio>,
    device: Option<String>,
) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    let emitter = app.clone();
    let engine = AudioEngine::start(device, move |chunk| {
        let _ = emitter.emit(
            "audio://chunk",
            ChunkEvent {
                timestamp_ms: chunk.timestamp_ms,
                sample_rate: chunk.sample_rate,
                rms: chunk.rms,
                is_voice: chunk.is_voice,
                samples: chunk.samples.len(),
            },
        );
    })?;
    *slot = Some(engine);
    Ok(())
}

/// Stop the running capture, if any. Idempotent.
#[tauri::command]
fn stop_capture(audio: tauri::State<'_, Audio>) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    Ok(())
}
