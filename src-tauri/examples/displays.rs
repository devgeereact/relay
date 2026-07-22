//! What Relay will actually call your screens — run it and compare against
//! System Settings › Displays.
//!
//!     cargo run --example displays
//!
//! This exists because the display picker is the control that decides which
//! physical screen a congregation sees, and it was offering "Monitor #1234555"
//! for a screen macOS calls "HP-532sf". Unit tests can pin the string handling
//! but cannot prove the AppKit lookup works on real hardware — only real
//! hardware can, so this prints both the raw OS name and the resolved one side
//! by side. A standalone binary rather than a test because `NSScreen` must be
//! read from the main thread, and that is where an example's `main` runs.

#[cfg(target_os = "macos")]
fn main() {
    use core_graphics::display::CGDisplayBounds;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::{ns_string, MainThreadMarker};

    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("not on the main thread — cannot read NSScreen");
        return;
    };

    let screens = NSScreen::screens(mtm);
    println!("{} display(s) attached\n", screens.len());

    for (i, screen) in screens.iter().enumerate() {
        let desc = screen.deviceDescription();
        let display_id = desc
            .objectForKey(ns_string!("NSScreenNumber"))
            .and_then(|n| n.downcast::<objc2_foundation::NSNumber>().ok())
            .map(|n| n.as_u32());

        let name = screen.localizedName().to_string();
        let scale = screen.backingScaleFactor();

        println!("[{i}] {name}");
        println!("     localizedName : {name}   <- what Relay now shows");
        match display_id {
            Some(id) => {
                let b = unsafe { CGDisplayBounds(id) };
                // Exactly the arithmetic tao uses for Monitor::position(), which
                // is the key Relay matches on.
                let key = (
                    (b.origin.x * scale).round() as i32,
                    (b.origin.y * scale).round() as i32,
                );
                println!("     CGDisplayID   : {id}");
                println!(
                    "     bounds        : {}x{} at ({}, {})  scale {scale}",
                    b.size.width as i32, b.size.height as i32, b.origin.x as i32, b.origin.y as i32
                );
                println!("     match key     : {key:?}   <- must equal Tauri's monitor position");
            }
            None => println!("     CGDisplayID   : <unavailable>"),
        }
        println!();
    }

    println!("If a name above differs from System Settings > Displays, the lookup is wrong.");
    println!("If a name reads 'Monitor #<digits>', the AppKit lookup failed and Relay fell");
    println!("back to a generic label — report that, it is the bug this was written for.");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("This probe is macOS-only. Elsewhere Relay uses the OS-reported name");
    println!("(Windows: \\\\.\\DISPLAY1, Linux: HDMI-1) via humanize_monitor_name.");
}
