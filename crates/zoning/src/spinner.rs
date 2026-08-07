//! A terminal progress indicator for work with no useful percentage.
//!
//! Every call in this binary that can genuinely take a moment — installing an
//! editor extension, walking a monorepo for `.zone` contracts — either returns
//! in milliseconds or takes long enough that silence reads as a hang. A
//! spinner is the cheap fix, and it is designed to cost nothing on the fast
//! path: [`Spinner::start`] renders nothing until its message has gone
//! unclaimed for a heartbeat (`GRACE`), so a `map` over three small
//! packages never flickers a single frame, while an editor CLI spawn that
//! takes seconds gets one immediately. Off a terminal, under `CI`, or with
//! `ZONING_NO_SETUP` set, it never spawns a thread at all — the same escape
//! hatch [`crate::setup::auto`] already honours, so one variable silences
//! every animation in the binary for a script capturing stderr.

use std::io::{IsTerminal as _, Write as _};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TICK: Duration = Duration::from_millis(80);
/// How long a call must run before anything renders.
///
/// Long enough that the fast path — a cached editor-setup state, a handful of
/// files — never draws a frame it would have to immediately erase.
const GRACE: Duration = Duration::from_millis(150);

/// A running spinner. Call [`Spinner::stop`] before printing the answer it was
/// standing in for; dropping without stopping clears the line just the same,
/// which is what an early `?` return wants.
pub struct Spinner {
    done: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Spinner {
    /// Start rendering `message` to standard error, or do nothing at all when
    /// the destination cannot show it or was asked not to.
    #[must_use]
    pub fn start(message: &str) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        if quiet() {
            return Self { done, thread: Mutex::new(None) };
        }
        let flag = Arc::clone(&done);
        let message = message.to_owned();
        let thread = std::thread::Builder::new()
            .name("zoning-spinner".to_owned())
            .spawn(move || spin(&flag, &message))
            .ok();
        Self { done, thread: Mutex::new(thread) }
    }

    /// Stop and clear the line. Idempotent, and safe from a shared reference,
    /// so every call site — including one holding the spinner only to pass it
    /// to a helper — can ask for silence without owning it outright.
    pub fn stop(&self) {
        self.done.store(true, Ordering::Relaxed);
        if let Ok(mut held) = self.thread.lock()
            && let Some(thread) = held.take()
        {
            let _ = thread.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn quiet() -> bool {
    std::env::var_os("CI").is_some()
        || std::env::var_os("ZONING_NO_SETUP").is_some()
        || !std::io::stderr().is_terminal()
}

fn spin(done: &AtomicBool, message: &str) {
    let start = Instant::now();
    let mut stderr = std::io::stderr();
    let mut shown = false;
    for frame in FRAMES.iter().cycle() {
        if done.load(Ordering::Relaxed) {
            break;
        }
        if shown || start.elapsed() >= GRACE {
            shown = true;
            let _ = write!(stderr, "\r{frame} {message}");
            let _ = stderr.flush();
        }
        std::thread::sleep(TICK);
    }
    if shown {
        let _ = write!(stderr, "\r{}\r", " ".repeat(message.chars().count() + 2));
        let _ = stderr.flush();
    }
}
