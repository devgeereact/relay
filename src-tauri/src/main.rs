// Relay — Tauri entry point.
//
// Phase 0/1 goal (see PROMPT.md): this should boot a blank window with the
// Svelte frontend loaded and nothing else. Wire up real commands as each
// module below gets built out — don't front-load functionality here.

mod audio;
mod channels;
mod db;
mod detection;
mod router;
mod stt;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running Relay");
}

// Placeholder command — delete once a real command exists (e.g. `get_service_status`).
// Kept only so the scaffold has one working example of the frontend<->backend bridge.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Relay is running. Hello, {name}.")
}
