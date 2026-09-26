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

    // THE BUILD MARKER (S13): `<short sha>[+dirty] <date>`, or `unknown`. Read
    // here, once, from git — a version is shared by every build of a branch and
    // the field audit for 2026-09-20 could not say which commit ran the service.
    // `rerun-if-changed` on HEAD and the packed refs so a checkout re-stamps it.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/packed-refs");
    println!("cargo:rustc-env=RELAY_BUILD_MARKER={}", build_marker());

    tauri_build::build();
}

fn build_marker() -> String {
    use std::process::Command;
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir("..")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let Some(sha) = git(&["rev-parse", "--short=9", "HEAD"]).filter(|s| !s.is_empty()) else {
        return "unknown".into();
    };
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    // The build DATE, from the system clock, in UTC — no crate for a timezone.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    // Civil-from-days (Howard Hinnant), enough for a date stamp.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{sha}{} {y:04}-{m:02}-{d:02}",
        if dirty { "+dirty" } else { "" }
    )
}
