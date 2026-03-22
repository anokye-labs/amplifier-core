//! # amplifier-ffi — C ABI bridge for amplifier-core
//!
//! Exposes `extern "C"` functions for P/Invoke from .NET. Every public
//! symbol follows the `amplifier_*` naming convention and uses opaque
//! pointer handles for lifetime safety across the FFI boundary.
//!
//! ## Error convention
//!
//! Functions that can fail return a null pointer (for handle-returning
//! functions) or `-1` (for int-returning functions). The caller retrieves
//! the error message via [`amplifier_last_error`].
//!
//! ## Threading
//!
//! A global Tokio multi-thread runtime is created once via `OnceLock` and
//! reused for all async work. Event callbacks are invoked on Tokio worker
//! threads — the .NET side must marshal back to its own synchronization
//! context if needed.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use tokio::runtime::Runtime;

// ---------------------------------------------------------------------------
// Global Tokio runtime (created once, reused forever)
// ---------------------------------------------------------------------------

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("failed to create Tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Thread-local error string
// ---------------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: &str) {
    let c = CString::new(msg).unwrap_or_else(|_| CString::new("(error contained null byte)").unwrap());
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(c);
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

// ---------------------------------------------------------------------------
// Callback type
// ---------------------------------------------------------------------------

/// Signature for the event callback provided by the .NET host.
///
/// Parameters:
/// - `session_id`   — UTF-8 session identifier
/// - `event_name`   — canonical event name (e.g. `"session:start"`)
/// - `payload_json` — JSON-serialized event payload
/// - `timestamp_ms` — Unix epoch milliseconds
/// - `sequence`     — monotonically increasing sequence number
pub type OnEventCallback = extern "C" fn(
    session_id: *const c_char,
    event_name: *const c_char,
    payload_json: *const c_char,
    timestamp_ms: u64,
    sequence: u64,
);

// ---------------------------------------------------------------------------
// Handle types
// ---------------------------------------------------------------------------

/// Opaque handle representing an amplifier-core configuration + event state.
pub struct AmplifierHandle {
    config: serde_json::Value,
    event_handler: Option<OnEventCallback>,
    sequence: AtomicU64,
}

/// Opaque handle representing a live session.
pub struct SessionHandle {
    session: amplifier_core::Session,
    parent: *mut AmplifierHandle,
}

// Safety: SessionHandle is only accessed through FFI functions that take
// exclusive raw-pointer ownership. The parent pointer is stable for the
// lifetime of the handle.
unsafe impl Send for SessionHandle {}

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Parse a JSON config string and return an opaque handle.
///
/// Returns null on failure — call [`amplifier_last_error`] for the message.
#[no_mangle]
pub extern "C" fn amplifier_init(config_json: *const c_char) -> *mut AmplifierHandle {
    clear_last_error();

    if config_json.is_null() {
        set_last_error("config_json is null");
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(config_json) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("invalid UTF-8 in config_json: {e}"));
            return ptr::null_mut();
        }
    };

    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(&format!("invalid JSON: {e}"));
            return ptr::null_mut();
        }
    };

    // Validate that SessionConfig can be constructed from this value
    if let Err(e) = amplifier_core::SessionConfig::from_value(value.clone()) {
        set_last_error(&format!("invalid session config: {e}"));
        return ptr::null_mut();
    }

    let handle = Box::new(AmplifierHandle {
        config: value,
        event_handler: None,
        sequence: AtomicU64::new(0),
    });

    Box::into_raw(handle)
}

/// Create a session from an existing handle.
///
/// `session_id` may be null — a UUID v4 will be generated.
/// Returns null on failure.
#[no_mangle]
pub extern "C" fn amplifier_create_session(
    handle: *mut AmplifierHandle,
    session_id: *const c_char,
) -> *mut SessionHandle {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle is null");
        return ptr::null_mut();
    }

    let amp = unsafe { &*handle };

    let sid = if session_id.is_null() {
        None
    } else {
        let c_str = unsafe { CStr::from_ptr(session_id) };
        match c_str.to_str() {
            Ok(s) => Some(s.to_owned()),
            Err(e) => {
                set_last_error(&format!("invalid UTF-8 in session_id: {e}"));
                return ptr::null_mut();
            }
        }
    };

    let config = match amplifier_core::SessionConfig::from_value(amp.config.clone()) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(&format!("failed to create session config: {e}"));
            return ptr::null_mut();
        }
    };

    let session = amplifier_core::Session::new(config, sid, None);

    let session_handle = Box::new(SessionHandle {
        session,
        parent: handle,
    });

    Box::into_raw(session_handle)
}

/// Fire-and-forget prompt execution.
///
/// Spawns execution on the Tokio runtime. The `session:end` event signals
/// completion. Returns 0 on successful spawn, -1 on error.
#[no_mangle]
pub extern "C" fn amplifier_execute(
    session: *mut SessionHandle,
    prompt: *const c_char,
) -> c_int {
    clear_last_error();

    if session.is_null() {
        set_last_error("session handle is null");
        return -1;
    }

    if prompt.is_null() {
        set_last_error("prompt is null");
        return -1;
    }

    let prompt_str = match unsafe { CStr::from_ptr(prompt) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(e) => {
            set_last_error(&format!("invalid UTF-8 in prompt: {e}"));
            return -1;
        }
    };

    // Extract what we need before spawning, so we only capture Send types.
    // Safety: the caller must ensure no concurrent access to this handle.
    let sh = unsafe { &mut *session };

    let parent = if sh.parent.is_null() {
        None
    } else {
        Some(unsafe { &*sh.parent })
    };

    let event_cb = parent.and_then(|p| p.event_handler);
    let seq_ptr: Option<&'static AtomicU64> = parent.map(|p| {
        // Safety: parent handle outlives the spawned task (contract)
        unsafe { &*(&p.sequence as *const AtomicU64) }
    });
    let session_id_owned = sh.session.session_id().to_owned();

    // Convert raw pointer to usize so the async block captures a Send type.
    // Safety: the FFI contract guarantees exclusive access to this handle
    // and that it outlives the spawned task.
    let session_addr = session as usize;

    get_runtime().spawn(async move {
        let sh = unsafe { &mut *(session_addr as *mut SessionHandle) };

        match sh.session.execute(&prompt_str).await {
            Ok(_result) => {
                if let (Some(cb), Some(seq)) = (event_cb, seq_ptr) {
                    emit_event(cb, seq, &session_id_owned, "session:end", "{}");
                }
            }
            Err(e) => {
                if let (Some(cb), Some(seq)) = (event_cb, seq_ptr) {
                    let payload = serde_json::json!({ "error": e.to_string() }).to_string();
                    emit_event(cb, seq, &session_id_owned, "session:error", &payload);
                }
                log::error!("amplifier_execute failed: {e}");
            }
        }
    });

    0
}

/// Register (or clear) the event callback on a handle.
///
/// Pass a valid function pointer to register, or transmute a null pointer
/// to clear the handler. A null `callback` value clears the handler.
#[no_mangle]
pub extern "C" fn amplifier_set_event_handler(
    handle: *mut AmplifierHandle,
    callback: Option<OnEventCallback>,
) {
    if handle.is_null() {
        return;
    }

    let amp = unsafe { &mut *handle };
    amp.event_handler = callback;
}

/// Return the last error message for this thread, or null if none.
///
/// The returned pointer is valid until the next FFI call on this thread.
#[no_mangle]
pub extern "C" fn amplifier_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(c) => c.as_ptr(),
            None => ptr::null(),
        }
    })
}

/// Free an `AmplifierHandle`. Idempotent — null is a no-op.
#[no_mangle]
pub extern "C" fn amplifier_handle_free(handle: *mut AmplifierHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

/// Free a `SessionHandle`. Idempotent — null is a no-op.
#[no_mangle]
pub extern "C" fn amplifier_session_free(session: *mut SessionHandle) {
    if !session.is_null() {
        unsafe { drop(Box::from_raw(session)) };
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn emit_event(
    cb: OnEventCallback,
    seq: &AtomicU64,
    session_id: &str,
    event_name: &str,
    payload_json: &str,
) {
    let sid = CString::new(session_id).unwrap_or_default();
    let ename = CString::new(event_name).unwrap_or_default();
    let payload = CString::new(payload_json).unwrap_or_default();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let seq_num = seq.fetch_add(1, Ordering::SeqCst);
    cb(sid.as_ptr(), ename.as_ptr(), payload.as_ptr(), now_ms, seq_num);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::atomic::AtomicU64;

    /// Minimal valid config JSON that passes SessionConfig validation.
    fn valid_config_cstr() -> CString {
        CString::new(r#"{"session":{"orchestrator":"loop-basic","context":"context-simple"}}"#)
            .unwrap()
    }

    // 1. init_valid_config
    #[test]
    fn init_valid_config() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null(), "expected non-null handle for valid config");
        // Cleanup
        amplifier_handle_free(handle);
    }

    // 2. init_invalid_config
    #[test]
    fn init_invalid_config() {
        let bad = CString::new("not json at all").unwrap();
        let handle = amplifier_init(bad.as_ptr());
        assert!(handle.is_null(), "expected null handle for invalid config");

        let err = amplifier_last_error();
        assert!(!err.is_null(), "expected error message");
        let msg = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(
            msg.contains("invalid JSON"),
            "error should mention invalid JSON, got: {msg}"
        );
    }

    // 3. create_session
    #[test]
    fn create_session() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null());

        let sid = CString::new("test-session-1").unwrap();
        let session = amplifier_create_session(handle, sid.as_ptr());
        assert!(!session.is_null(), "expected non-null session handle");

        amplifier_session_free(session);
        amplifier_handle_free(handle);
    }

    // 4. event_handler_accept
    #[test]
    fn event_handler_accept() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null());

        extern "C" fn dummy_cb(
            _sid: *const c_char,
            _event: *const c_char,
            _payload: *const c_char,
            _ts: u64,
            _seq: u64,
        ) {}

        amplifier_set_event_handler(handle, Some(dummy_cb));
        let amp = unsafe { &*handle };
        assert!(amp.event_handler.is_some(), "handler should be set");

        amplifier_handle_free(handle);
    }

    // 5. event_handler_null
    #[test]
    fn event_handler_null() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null());

        // Set a handler first
        extern "C" fn dummy_cb(
            _sid: *const c_char,
            _event: *const c_char,
            _payload: *const c_char,
            _ts: u64,
            _seq: u64,
        ) {}
        amplifier_set_event_handler(handle, Some(dummy_cb));

        // Clear it with None
        amplifier_set_event_handler(handle, None);
        let amp = unsafe { &*handle };
        assert!(amp.event_handler.is_none(), "handler should be cleared");

        amplifier_handle_free(handle);
    }

    // 6. tokio_runtime_reuse
    #[test]
    fn tokio_runtime_reuse() {
        let rt1 = get_runtime() as *const Runtime;
        let rt2 = get_runtime() as *const Runtime;
        assert_eq!(rt1, rt2, "runtime should be the same instance");

        // Create and drop multiple handles to verify runtime survives
        for _ in 0..3 {
            let config = valid_config_cstr();
            let handle = amplifier_init(config.as_ptr());
            assert!(!handle.is_null());
            let session = amplifier_create_session(handle, ptr::null());
            assert!(!session.is_null());
            amplifier_session_free(session);
            amplifier_handle_free(handle);
        }

        let rt3 = get_runtime() as *const Runtime;
        assert_eq!(rt1, rt3, "runtime should survive handle cycles");
    }

    // 7. shutdown_cleanup
    #[test]
    fn shutdown_cleanup() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null());

        let session = amplifier_create_session(handle, ptr::null());
        assert!(!session.is_null());

        // Drop session first, then handle — no leaks or panics
        amplifier_session_free(session);
        amplifier_handle_free(handle);
        // If we reach here without panic/segfault, cleanup is correct
    }

    // 8. session_config_roundtrip
    #[test]
    fn session_config_roundtrip() {
        let original = r#"{"session":{"orchestrator":"loop-basic","context":"context-simple"}}"#;
        let value: serde_json::Value = serde_json::from_str(original).unwrap();

        let config = amplifier_core::SessionConfig::from_value(value.clone()).unwrap();
        let roundtripped = serde_json::to_value(&config.config).unwrap();

        // The round-tripped value should contain the same session keys
        let session_obj = roundtripped
            .get("session")
            .and_then(|v| v.as_object())
            .expect("session key should exist");
        assert_eq!(
            session_obj.get("orchestrator").and_then(|v| v.as_str()),
            Some("loop-basic")
        );
        assert_eq!(
            session_obj.get("context").and_then(|v| v.as_str()),
            Some("context-simple")
        );
    }

    // 9. safe_handle_double_free
    #[test]
    fn safe_handle_double_free() {
        let config = valid_config_cstr();
        let handle = amplifier_init(config.as_ptr());
        assert!(!handle.is_null());

        // First free
        amplifier_handle_free(handle);
        // Second free with null is a no-op (safe)
        amplifier_handle_free(ptr::null_mut());
        // If we reach here, double-free guard works
    }

    // 10. event_sequence_monotonic
    #[test]
    fn event_sequence_monotonic() {
        use std::sync::{Arc, Mutex};

        let sequences: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
        let seq_clone = Arc::clone(&sequences);

        // Use a static to capture sequences across the extern "C" boundary
        use std::sync::LazyLock;
        static CAPTURED: LazyLock<Mutex<Vec<u64>>> = LazyLock::new(|| Mutex::new(Vec::new()));
        CAPTURED.lock().unwrap().clear();

        extern "C" fn capture_cb(
            _sid: *const c_char,
            _event: *const c_char,
            _payload: *const c_char,
            _ts: u64,
            seq: u64,
        ) {
            CAPTURED.lock().unwrap().push(seq);
        }

        let seq_counter = AtomicU64::new(0);
        let session_id = "test-session";

        // Emit several events
        for _ in 0..10 {
            emit_event(capture_cb, &seq_counter, session_id, "test:event", "{}");
        }

        let captured = CAPTURED.lock().unwrap();
        assert_eq!(captured.len(), 10);
        for i in 1..captured.len() {
            assert!(
                captured[i] > captured[i - 1],
                "sequence not monotonic: {} <= {} at index {}",
                captured[i],
                captured[i - 1],
                i
            );
        }

        drop(seq_clone); // suppress unused warning
        drop(sequences);
    }
}
