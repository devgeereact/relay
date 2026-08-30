//! Keep the machine — and the projector — awake while Relay is working.
//!
//! Single responsibility: hold an OS "do not sleep the display" assertion for
//! exactly as long as Relay is doing something a congregation can see, and
//! release it the moment it is not.
//!
//! ## Why this is not a nicety
//!
//! Every failure this prevents happens in front of people and cannot be undone:
//!
//! * The **projector goes black** mid-sermon because the operator has not touched
//!   the trackpad for fifteen minutes. Relay is running perfectly; the wall is
//!   dark. Nothing in the app is wrong, so nothing in the app says anything.
//! * The **machine sleeps** and takes the microphone with it. Capture stops, the
//!   transcript stops, and the first sign is that Relay has "stopped working".
//! * A display asleep at the OS level also stops the output window painting, so
//!   `OutputHealth` correctly reports a screen that is not answering — a true
//!   alarm about a cause the operator will not guess.
//!
//! A church laptop is very often a personal laptop with default power settings.
//! Asking a volunteer to find the energy-saver pane before a service is not a
//! plan; the software that needs the screen should say so.
//!
//! ## The rule about when it is held
//!
//! Held while **the microphone is live, a service is recording, or an output
//! window is open** — any one of the three. Released when none of them is true.
//!
//! Not held merely because Relay is running. An app that quietly disables sleep
//! for as long as it is open is an app that flattens a battery in a bag, and the
//! next person to trust it with a power setting will be right not to.
//!
//! ## No new dependency
//!
//! Both platforms expose this in their base system libraries, so this speaks to
//! them directly rather than pulling a crate onto the offline-first build:
//!
//! * **macOS** — `IOPMAssertionCreateWithName` with `PreventUserIdleDisplaySleep`,
//!   which keeps the display up (and therefore the system too). Released by id.
//! * **Windows** — `SetThreadExecutionState` with `ES_CONTINUOUS |
//!   ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED`, cleared by setting
//!   `ES_CONTINUOUS` alone.
//!
//! Everything else is a no-op that reports honestly rather than pretending.

use std::sync::Mutex;

/// What Relay is currently doing that needs the screen up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Need {
    pub capturing: bool,
    pub service_recording: bool,
    pub outputs_open: bool,
}

impl Need {
    /// Any one of the three is enough. Written as a method rather than inline so
    /// the rule has one home and a test can state it.
    pub fn wants_awake(&self) -> bool {
        self.capturing || self.service_recording || self.outputs_open
    }
}

/// Whether the assertion is currently held, and the platform handle for it.
///
/// `Option<u32>` on macOS is the IOKit assertion id; on Windows the flag is
/// thread-global and needs no handle, so the bool alone carries it.
#[derive(Default)]
struct State {
    held: bool,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    assertion: u32,
    /// Set when the platform call failed. Reported, never retried in a loop — a
    /// machine that refuses the assertion will refuse it every time, and a
    /// per-second retry would be a log flood during a service.
    failed: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Apply `need`. Idempotent: calling it with an unchanged need does nothing, so
/// it is safe to call from every place that changes one of the three facts.
///
/// Never returns an error. A machine that will not hold the assertion is a
/// machine where the operator has to set power settings by hand, and that is
/// worth a line on the diagnostics screen — it is not worth failing a command
/// that was really about starting a microphone.
pub fn apply(need: Need) {
    let want = need.wants_awake();
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let st = guard.get_or_insert_with(State::default);
    if want == st.held {
        return;
    }
    if want {
        match acquire() {
            Ok(id) => {
                st.assertion = id;
                st.held = true;
                st.failed = false;
            }
            Err(e) => {
                st.failed = true;
                eprintln!("wake: could not keep the display awake: {e}");
            }
        }
    } else {
        release(st.assertion);
        st.held = false;
        st.assertion = 0;
    }
}

/// Is the assertion held right now? Reported in the diagnostic bundle, because
/// "the projector went black" is answered differently depending on it.
pub fn is_held() -> bool {
    STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.held))
        .unwrap_or(false)
}

/// Did the last attempt to hold it fail? Distinct from "not held": one is a
/// choice and the other is a machine that will sleep during a service.
pub fn failed() -> bool {
    STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.failed))
        .unwrap_or(false)
}

// ── macOS ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod sys {
    use std::ffi::c_void;

    #[repr(C)]
    struct __CFString(c_void);
    type CFStringRef = *const __CFString;

    // kIOPMAssertionLevelOn
    const LEVEL_ON: u32 = 255;
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            alloc: *const c_void,
            bytes: *const u8,
            num_bytes: isize,
            encoding: u32,
            is_external: u8,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    unsafe fn cfstr(s: &str) -> CFStringRef {
        unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                s.as_ptr(),
                s.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
                0,
            )
        }
    }

    pub fn acquire() -> Result<u32, String> {
        // PreventUserIdleDisplaySleep keeps the DISPLAY up, which is the one that
        // matters: a system-only assertion still lets the projector go black.
        unsafe {
            let ty = cfstr("PreventUserIdleDisplaySleep");
            // Shown verbatim in `pmset -g assertions`, so make it say why.
            let name = cfstr("Relay is running a service");
            if ty.is_null() || name.is_null() {
                return Err("could not build the assertion strings".into());
            }
            let mut id: u32 = 0;
            let rc = IOPMAssertionCreateWithName(ty, LEVEL_ON, name, &mut id);
            CFRelease(ty as *const _);
            CFRelease(name as *const _);
            if rc == 0 {
                Ok(id)
            } else {
                Err(format!("IOPMAssertionCreateWithName failed ({rc})"))
            }
        }
    }

    pub fn release(id: u32) {
        if id != 0 {
            unsafe {
                IOPMAssertionRelease(id);
            }
        }
    }
}

// ── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod sys {
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
    const ES_CONTINUOUS: u32 = 0x8000_0000;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }

    pub fn acquire() -> Result<u32, String> {
        // ES_CONTINUOUS makes the state stick until it is cleared, rather than
        // resetting the idle timer once — the difference between "the screen
        // stays up for the service" and "the screen stays up for a moment".
        let prev = unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED)
        };
        if prev == 0 {
            Err("SetThreadExecutionState refused the request".into())
        } else {
            Ok(1)
        }
    }

    pub fn release(_id: u32) {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

// ── Anywhere else ───────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod sys {
    pub fn acquire() -> Result<u32, String> {
        // Not silently "fine". Relay ships on two platforms; a third would need
        // its own answer (org.freedesktop.ScreenSaver on Linux), and reporting
        // that plainly is better than a stub that reads as success.
        Err("keeping the display awake is not implemented on this platform".into())
    }
    pub fn release(_id: u32) {}
}

use sys::{acquire, release};

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY DOOR THAT OPENS OR CLOSES A SCREEN MUST TELL THIS MODULE.
    ///
    /// Written after this exact rule was broken by the commit that introduced the
    /// module. `refresh_wake` was added at the call sites — `open_channel_output`
    /// and `close_channel_output` — and **two of the three functions that open a
    /// native output window were missed**, including `auto_open_outputs`, which
    /// `App.svelte` calls on mount and is therefore the path that runs at *every
    /// launch*. Outputs came back by themselves after a restart and nothing told
    /// the OS to keep the display up: the precise failure this module exists to
    /// prevent, on the most common path there is.
    ///
    /// CLAUDE.md rule 36 says the check belongs at the choke point, and the choke
    /// point here is inside `channels` — which cannot ask whether a microphone is
    /// live or a service is recording, because those types live in `main`. So the
    /// call sites stay, and this test enumerates them instead of a person doing it
    /// from memory. Same shape as
    /// `servicelock::every_protected_command_actually_guards_itself`.
    #[test]
    fn every_function_that_opens_or_closes_a_screen_refreshes_the_wake_state() {
        let src = include_str!("main.rs");
        let mut offenders = Vec::new();
        // Walk the file function by function; a `fn` at column 0 starts a new one.
        let mut current = "<top of file>";
        let mut body = String::new();
        let mut touches_windows = false;
        let flush = |name: &str, body: &str, touches: bool, out: &mut Vec<String>| {
            if touches && !body.contains("refresh_wake(") {
                out.push(name.to_string());
            }
        };
        for line in src.lines() {
            if line.starts_with("fn ") || line.starts_with("async fn ") {
                flush(current, &body, touches_windows, &mut offenders);
                current = line.trim_start_matches("async ").trim_start_matches("fn ");
                body.clear();
                touches_windows = false;
            }
            if line.contains("channels::open_native_window(")
                || line.contains("channels::close_window(")
            {
                touches_windows = true;
            }
            body.push_str(line);
            body.push('\n');
        }
        flush(current, &body, touches_windows, &mut offenders);

        assert!(
            offenders.is_empty(),
            "these open or close an output window and never call `refresh_wake`, so \
             the display can sleep with a screen live: {offenders:?}"
        );
    }

    /// The rule, stated once. Any of the three; none means release.
    #[test]
    fn any_one_of_the_three_wants_the_screen_up() {
        assert!(!Need::default().wants_awake());
        for n in [
            Need {
                capturing: true,
                ..Default::default()
            },
            Need {
                service_recording: true,
                ..Default::default()
            },
            Need {
                outputs_open: true,
                ..Default::default()
            },
        ] {
            assert!(n.wants_awake(), "{n:?}");
        }
    }

    /// Relay running is NOT a reason to hold it.
    ///
    /// An app that disables sleep for as long as it is open flattens a battery in
    /// a bag, and the next person to distrust it with a power setting is right.
    #[test]
    fn merely_being_open_does_not_keep_the_machine_awake() {
        assert!(!Need::default().wants_awake());
    }

    /// Idempotent, and it really does take and release on this platform.
    ///
    /// Skipped nowhere: if the platform refuses the assertion, `failed()` says so
    /// and the test asserts we reported it rather than claiming success — a
    /// control that cannot detect its own failure is the thing this repository
    /// keeps finding.
    #[test]
    fn taking_and_releasing_is_idempotent_and_honest() {
        apply(Need {
            capturing: true,
            ..Default::default()
        });
        let held = is_held();
        assert!(
            held || failed(),
            "neither held nor reported as failed — that combination is the lie"
        );
        apply(Need {
            capturing: true,
            ..Default::default()
        }); // no change
        assert_eq!(is_held(), held);

        apply(Need::default());
        assert!(!is_held(), "released when nothing needs it");
        apply(Need::default());
        assert!(!is_held());
    }
}
