//! The one error type that crosses the IPC bridge.
//!
//! ## Why this exists
//!
//! Every Tauri command returned `Result<_, String>` — 69 of them — built by
//! `map_err(|e| e.to_string())`. So what reached the frontend was a raw Rust error
//! string with no structure at all, and the console could not tell
//!
//!   * "that verse isn't in your Bible"        (the operator can fix this)
//!   * "the database is busy, try again"       (retrying works)
//!   * "the disk is full"                      (retrying will never work)
//!   * "a service is being recorded"           (a deliberate refusal, not a fault)
//!
//! apart from each other. They were all just… text. Which is exactly why
//! `Channels.svelte` ended up rendering `String(err)` in a monospace font to a church
//! volunteer: given nothing but a sentence, there is nothing else you can do with it.
//!
//! ## The shape
//!
//! Serialises as `{ kind, message }`, so the frontend can branch on `kind` and still
//! always have something to show. `src/lib/errors.js` is the only place that decides
//! what a volunteer reads.
//!
//! ## `Internal` is not a cop-out, it is a promise kept honestly
//!
//! An error we cannot classify is reported AS unclassified rather than being guessed
//! at. A `From<String>` that silently labelled every legacy `map_err(…to_string())`
//! as a user-fixable refusal would be worse than the strings were: it would look like
//! structure while lying about it.

use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    /// A deliberate refusal. The message is already written for a volunteer, and the
    /// operator can act on it: "a service is being recorded", "plan needs a title".
    /// NOT a fault — nothing is broken.
    Refused { message: String },

    /// Asked for something that is not there. A reference outside the corpus, a plan
    /// that was deleted in another window.
    NotFound { message: String },

    /// The database is locked or busy. **Retrying is reasonable** — and that is the
    /// whole point of distinguishing it, because retrying a disk-full error is not.
    Busy { message: String },

    /// The disk, the network, or the OS said no. Retrying will usually not help.
    Io { message: String },

    /// We do not know what this is. Reported as such — see the module doc.
    Internal { message: String },
}

impl Error {
    pub fn refused(message: impl Into<String>) -> Self {
        Error::Refused {
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Error::NotFound {
            message: message.into(),
        }
    }

    /// The sentence, whatever the kind.
    pub fn message(&self) -> &str {
        match self {
            Error::Refused { message }
            | Error::NotFound { message }
            | Error::Busy { message }
            | Error::Io { message }
            | Error::Internal { message } => message,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

/// SQLite errors carry the one distinction the operator most needs: *is it worth
/// pressing the button again?*
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ffi::ErrorCode;
        match &e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound {
                message: "That isn't in Relay's library.".into(),
            },
            rusqlite::Error::SqliteFailure(f, _)
                if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) =>
            {
                Error::Busy {
                    message: "Relay is busy saving. Try that again in a moment.".into(),
                }
            }
            _ => Error::Internal {
                message: e.to_string(),
            },
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io {
            message: e.to_string(),
        }
    }
}

/// A poisoned lock means another thread panicked while holding it. That is a bug,
/// never something the operator did.
impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        Error::Internal {
            message: format!("internal lock error: {e}"),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Internal {
            message: e.to_string(),
        }
    }
}

impl From<tauri::Error> for Error {
    fn from(e: tauri::Error) -> Self {
        Error::Internal {
            message: e.to_string(),
        }
    }
}

/// The bridge from the modules that still speak `Result<_, String>` (audio, stt,
/// channels, models, proimport).
///
/// Deliberately `Internal`, NOT `Refused`. A bare string is an error nobody has
/// classified yet, and saying so is the honest answer. Classifying it as something
/// the operator can fix would be a lie with a nicer face — and those modules' errors
/// are, in fact, mostly device and network failures.
impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Internal { message }
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Error::Internal {
            message: message.into(),
        }
    }
}

/// Command results. `Result<T>` throughout `main.rs`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: the frontend must be able to tell "press it again" from
    /// "pressing it again will never work".
    #[test]
    fn a_busy_database_is_distinguishable_from_a_broken_one() {
        let busy: Error = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".into()),
        )
        .into();
        assert!(matches!(busy, Error::Busy { .. }));

        let broken: Error = rusqlite::Error::InvalidQuery.into();
        assert!(matches!(broken, Error::Internal { .. }));
    }

    #[test]
    fn a_missing_row_is_not_found_not_a_crash() {
        let e: Error = rusqlite::Error::QueryReturnedNoRows.into();
        assert!(matches!(e, Error::NotFound { .. }));
    }

    /// An unclassified string error must NOT masquerade as a refusal the operator can
    /// act on. It is unclassified, and it says so.
    #[test]
    fn a_bare_string_is_internal_never_a_refusal() {
        let e: Error = "the audio device vanished".to_string().into();
        assert!(matches!(e, Error::Internal { .. }), "{e:?}");
    }

    #[test]
    fn a_refusal_is_explicit_and_keeps_its_sentence() {
        let e = Error::refused("A service is being recorded. End it before rehearsing.");
        assert!(matches!(e, Error::Refused { .. }));
        assert!(e.message().contains("End it before rehearsing"));
    }

    /// It crosses the bridge as `{ kind, message }` — both halves, always.
    #[test]
    fn it_serialises_with_a_kind_the_frontend_can_branch_on() {
        let json = serde_json::to_value(Error::not_found("no such verse")).unwrap();
        assert_eq!(json["kind"], "not_found");
        assert_eq!(json["message"], "no such verse");

        let json = serde_json::to_value(Error::refused("nope")).unwrap();
        assert_eq!(json["kind"], "refused");
    }

    /// Every variant must carry a sentence. An error with a `kind` and nothing to show
    /// the operator is half an error.
    #[test]
    fn every_kind_carries_a_message() {
        for e in [
            Error::refused("a"),
            Error::not_found("b"),
            Error::Busy {
                message: "c".into(),
            },
            Error::Io {
                message: "d".into(),
            },
            Error::Internal {
                message: "e".into(),
            },
        ] {
            assert!(!e.message().is_empty());
            assert_eq!(e.to_string(), e.message());
        }
    }
}
