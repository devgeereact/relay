use std::path::Path;

fn main() {
    // channels.rs embeds ../dist via include_dir! at compile time. On a fresh
    // clone (before `npm run build`) that directory may not exist, which would
    // fail the build. Ensure it exists with a placeholder so compilation always
    // succeeds; a real build overwrites it. The LAN output/stage server then
    // 404s until the frontend is built, rather than breaking the whole build.
    let dist = Path::new("../dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(dist);
    }
    if !dist.join("output.html").exists() {
        let _ = std::fs::write(
            dist.join("output.html"),
            "<!doctype html><meta charset=utf-8><title>Relay Output</title>Run `npm run build` to generate the output page.",
        );
    }
    println!("cargo:rerun-if-changed=../dist");

    tauri_build::build();
}
