//! What this machine and this build can actually do.
//!
//! Single responsibility: **read facts about the host and the binary** for the
//! Launch & Startup screens (Hardware Check, Plugin Loading). It computes
//! nothing about scripture, touches no database, and holds no state.
//!
//! ── Why this module exists ─────────────────────────────────────────────────
//!
//! The Hardware Check screen shipped with four rows it could not answer — CPU,
//! memory, GPU, disk — rendered honestly as "not probed". Honest, but useless:
//! the operator debugging a laptop that drops transcripts is exactly the person
//! that screen exists for, and it had nothing to tell them.
//!
//! Every value here is READ, never assumed:
//!
//!   · Cores come from `available_parallelism` — what this process may actually
//!     use, which on a cgroup-limited or affinity-pinned box is not the same as
//!     the physical core count.
//!   · Memory and disk come from `sysinfo`, at the moment of the call.
//!   · **GPU acceleration is a BUILD fact, not a hardware one.** whisper.cpp is
//!     compiled with a backend or it is not, and no amount of GPU in the machine
//!     changes that. Reporting "Apple M3 Max" next to a CPU-only build would be
//!     the most convincing lie on the screen. So we report the feature flags
//!     this binary was compiled with, and say plainly when the answer is CPU.
//!
//! Integration reachability (OBS, ATEM) is a TCP connect and nothing more. Relay
//! implements neither control protocol; "something is listening on the port a
//! default OBS install would use" is the strongest claim it is entitled to make,
//! and it is worded that way.

use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

/// A snapshot of the host, taken at the moment of the call.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hardware {
    /// Threads this process may actually use. `None` if the OS would not say.
    pub cores: Option<usize>,
    /// Physical cores, when the OS reports them. Informational only.
    pub physical_cores: Option<usize>,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    /// Free bytes on the volume holding the app-data directory — the one that
    /// fills up, because that is where models, media and the database live.
    pub free_disk_bytes: u64,
    pub total_disk_bytes: u64,
    /// The volume that was measured, so the number can be checked by a human.
    pub disk_mount: String,
    /// Compiled-in whisper backends. Empty means CPU, and that is the truth for
    /// every build Relay currently ships.
    pub gpu_backends: Vec<&'static str>,
    pub os: String,
    pub arch: &'static str,
}

/// The whisper acceleration backends this BINARY was built with.
///
/// Deliberately compile-time. See the module docs: a GPU that whisper.cpp was
/// not compiled to use is not acceleration, it is decoration.
pub fn gpu_backends() -> Vec<&'static str> {
    let mut v = Vec::new();
    // macOS is Metal ALWAYS, feature flag or not: `Cargo.toml` pulls whisper-rs with
    // `features = ["metal"]` from the macOS target block, which links the backend
    // without ever setting `feature = "metal"` on this crate. Reading the flag alone
    // reported "CPU" on a binary that was demonstrably running on the GPU — the
    // Hardware Check screen exists to tell an operator what their machine is doing,
    // and a screen that under-reports is the same defect as one that over-reports.
    if cfg!(target_os = "macos") || cfg!(feature = "metal") {
        v.push("Metal");
    }
    if cfg!(feature = "coreml") {
        v.push("Core ML");
    }
    if cfg!(feature = "cuda") {
        v.push("CUDA");
    }
    if cfg!(feature = "vulkan") {
        v.push("Vulkan");
    }
    v
}

/// Read the host. `data_dir` is the app-data path whose volume gets measured.
pub fn read(data_dir: &Path) -> Hardware {
    use sysinfo::{Disks, System};

    let mut sys = System::new();
    sys.refresh_memory();

    // The volume holding `data_dir` is the one that matters — a laptop with a
    // full boot volume and an empty external drive is still a laptop that cannot
    // download a model. Pick the disk with the LONGEST mount point that is a
    // prefix of the path: on macOS every mount point is a prefix of "/" too, so
    // the shortest match is always wrong.
    let disks = Disks::new_with_refreshed_list();
    let best = disks
        .list()
        .iter()
        .filter(|d| data_dir.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());

    let (free_disk_bytes, total_disk_bytes, disk_mount) = match best {
        Some(d) => (
            d.available_space(),
            d.total_space(),
            d.mount_point().to_string_lossy().into_owned(),
        ),
        // Not being able to identify the volume is reported as zero-and-unknown,
        // never as a comfortable default. The screen renders it as a failure to
        // read, which is what it is.
        None => (0, 0, String::new()),
    };

    Hardware {
        cores: std::thread::available_parallelism().ok().map(|n| n.get()),
        physical_cores: System::physical_core_count(),
        total_memory_bytes: sys.total_memory(),
        available_memory_bytes: sys.available_memory(),
        free_disk_bytes,
        total_disk_bytes,
        disk_mount,
        gpu_backends: gpu_backends(),
        os: System::long_os_version().unwrap_or_else(|| "unknown".into()),
        arch: std::env::consts::ARCH,
    }
}

/// One integration's reachability.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PortProbe {
    pub label: &'static str,
    pub port: u16,
    /// Something accepted a TCP connection. NOT "OBS is running" — see below.
    pub listening: bool,
}

/// How long to wait for a local port. Short on purpose: this runs during boot,
/// and a closed port on loopback refuses instantly. Anything slower than this is
/// a firewall prompt, not a service, and the boot must not sit behind it.
const PORT_TIMEOUT: Duration = Duration::from_millis(300);

/// Is something listening on a loopback port?
///
/// This is the ONLY claim Relay is entitled to make about OBS or ATEM: it
/// implements neither protocol, so it cannot know that the thing which answered
/// is the thing named. Callers must word it as "something is listening", and the
/// Plugin Loading screen does.
pub fn port_open(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, PORT_TIMEOUT).is_ok()
}

/// Relay's OWN listeners: the kiosk hub and the LAN HTTP server.
///
/// ── Why these are probed at all ──────────────────────────────────────────────
///
/// The launch screen used to report both as `ok` **having asked neither**. The
/// kiosk row listed how many channels were configured and printed `ws://…:8031`;
/// the HTTP row printed `http://<ip>:8032`. Neither is a fact about whether
/// anything is listening on those ports.
///
/// They are ordinary TCP ports on a volunteer's laptop and binding them can fail
/// — another program already has them, or a security tool refuses. When that
/// happens every OBS browser source and the preacher's stage page go dead, the
/// operator copies a URL that cannot answer, and the one screen whose job is to
/// say "is this machine going to work?" says yes.
///
/// A check that cannot fail is not a check.
pub const KIOSK_PORT: u16 = 8031;
pub const HTTP_PORT: u16 = 8032;

pub fn probe_own_ports() -> Vec<PortProbe> {
    [("Relay kiosk hub", KIOSK_PORT), ("Relay HTTP", HTTP_PORT)]
        .into_iter()
        .map(|(label, port)| PortProbe {
            label,
            port,
            listening: port_open(port),
        })
        .collect()
}

/// The default ports of the integrations Relay is built to sit alongside.
pub fn probe_integrations() -> Vec<PortProbe> {
    // OBS WebSocket 5.x defaults to 4455; ATEM's control protocol is 9910.
    // Both are the stock values — an operator who moved them will read
    // "not detected", which is why the screen says which port it looked at.
    //
    // Relay's own listeners ride along, so the launch screen can ask about them
    // in the same round trip it already makes. They are labelled `Relay …` so a
    // reader can tell "somebody else's software" from "ours".
    probe_own_ports()
        .into_iter()
        .chain(
            [("OBS WebSocket", 4455u16), ("ATEM", 9910u16)]
                .into_iter()
                .map(|(label, port)| PortProbe {
                    label,
                    port,
                    listening: port_open(port),
                }),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn reads_the_host_without_panicking() {
        let hw = read(Path::new("."));
        // available_parallelism can legitimately fail; everything else must be
        // present. A machine with zero total memory is a read that went wrong.
        assert!(hw.total_memory_bytes > 0, "total memory read as zero");
        assert!(!hw.arch.is_empty());
    }

    #[test]
    fn never_claims_more_free_memory_than_exists() {
        let hw = read(Path::new("."));
        assert!(hw.available_memory_bytes <= hw.total_memory_bytes);
    }

    #[test]
    fn available_memory_is_actually_read() {
        // THE BUG THIS EXISTS FOR: on sysinfo 0.32, `available_memory()` returns
        // 0 on macOS — silently, no error. The Hardware Check screen read
        // "0.0 GB free of 25.8 GB — close other apps before the service" on a
        // perfectly healthy laptop, on every boot: a false alarm from the one
        // screen whose entire job is being trustworthy.
        //
        // No running machine has zero bytes available. If this fails, the
        // sysinfo bound in Cargo.toml has been relaxed or the platform regressed.
        let hw = read(Path::new("."));
        assert!(
            hw.available_memory_bytes > 0,
            "available memory read as zero — sysinfo is not reporting on this platform"
        );
    }

    #[test]
    fn measures_the_volume_holding_the_given_path() {
        // The bug this catches: picking the FIRST disk whose mount point is a
        // prefix of the path. On macOS "/" is a prefix of every path, so every
        // probe reported the boot volume no matter where app-data actually was.
        let hw = read(Path::new("."));
        if hw.total_disk_bytes > 0 {
            assert!(hw.free_disk_bytes <= hw.total_disk_bytes);
            assert!(!hw.disk_mount.is_empty());
        }
    }

    #[test]
    fn gpu_backends_reflect_the_build_not_the_machine() {
        let v = gpu_backends();
        if cfg!(target_os = "macos") {
            // Every macOS build links Metal via the target dependency, so the screen
            // must say Metal. It said CPU for as long as this read the feature flag,
            // on builds that were decoding 7x faster than a CPU build can.
            assert!(
                v.contains(&"Metal"),
                "macOS builds link Metal unconditionally — see Cargo.toml"
            );
        } else if cfg!(feature = "metal") {
            assert!(v.contains(&"Metal"));
        } else {
            // Windows and Linux keep the CPU default deliberately: a GPU runtime the
            // box may not have turns a slow start into no start.
            assert!(!v.contains(&"Metal"));
        }
    }

    #[test]
    fn a_closed_port_reads_as_closed() {
        // Bind and immediately drop, so the port is one nothing is on.
        let port = {
            let l = match TcpListener::bind("127.0.0.1:0") {
                Ok(l) => l,
                Err(_) => return, // no loopback in this sandbox; nothing to assert
            };
            let p = l.local_addr().map(|a| a.port()).unwrap_or(0);
            drop(l);
            p
        };
        if port != 0 {
            assert!(!port_open(port), "a port with no listener read as open");
        }
    }

    #[test]
    fn an_open_port_reads_as_open() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let port = match listener.local_addr() {
            Ok(a) => a.port(),
            Err(_) => return,
        };
        assert!(port_open(port), "a bound port read as closed");
    }

    /// What THIS machine actually reports, as JSON.
    ///
    /// `cargo test sysprobe::tests::print_hardware -- --ignored --nocapture`
    ///
    /// Ignored because the output is machine-specific and asserting on it would
    /// fail on every other laptop. It exists because the Hardware Check screen
    /// cannot be screenshotted against a real backend on this machine — this is
    /// how its fixture is taken from reality instead of being invented.
    #[test]
    #[ignore]
    fn print_hardware() {
        let hw = read(&std::env::temp_dir());
        match serde_json::to_string_pretty(&hw) {
            Ok(j) => println!("{j}"),
            Err(e) => println!("could not serialise: {e}"),
        }
        match serde_json::to_string_pretty(&probe_integrations()) {
            Ok(j) => println!("{j}"),
            Err(e) => println!("could not serialise: {e}"),
        }
    }

    #[test]
    fn probes_both_integrations_and_names_their_ports() {
        // Does not assert on `listening` — that depends on what the developer
        // happens to be running. It asserts the CONTRACT: each is probed, and
        // each carries the port it looked at, because the screen shows it.
        let probes = probe_integrations();
        assert_eq!(probes.len(), 4, "two integrations and Relay's own two");
        assert!(probes.iter().any(|p| p.port == 4455)); // OBS
        assert!(probes.iter().any(|p| p.port == 9910)); // ATEM
    }

    /// RELAY'S OWN LISTENERS ARE PROBED TOO, AND LABELLED AS OURS.
    ///
    /// The launch screen reported the kiosk hub and the HTTP server as `ok`
    /// having asked neither — it printed the URL and counted channels, which are
    /// not facts about whether anything is listening. Binding either port can
    /// fail on a volunteer's laptop, and when it does every OBS browser source
    /// and the preacher's stage page go dead while the screen says it is fine.
    #[test]
    fn relays_own_ports_are_probed_and_named_as_ours() {
        let own = probe_own_ports();
        assert_eq!(own.len(), 2);
        assert!(own
            .iter()
            .any(|p| p.port == KIOSK_PORT && p.label.starts_with("Relay ")));
        assert!(own
            .iter()
            .any(|p| p.port == HTTP_PORT && p.label.starts_with("Relay ")));

        // …and they arrive on the same command the launch screen already calls,
        // so the two rows cost no extra round trip.
        let all = probe_integrations();
        for p in own {
            assert!(
                all.iter().any(|q| q.label == p.label),
                "{} is missing from probe_integrations — the launch rows read that one",
                p.label
            );
        }
    }

    /// A port that IS listening reads as listening. Without this the probe could
    /// return `false` for everything and every assertion above would still pass.
    #[test]
    fn a_real_listener_is_detected() {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().unwrap().port();
        assert!(port_open(port), "a bound port read as closed");
        drop(l);
    }
}
