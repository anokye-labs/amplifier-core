//! Session create/initialize/execute/cleanup/destroy for FFI.
//!
//! Provides `amplifier_session_create`, `amplifier_session_initialize`,
//! `amplifier_session_execute`, `amplifier_session_cleanup`, and
//! `amplifier_session_destroy` for managing session lifecycle across the FFI
//! boundary.

use std::ffi::{CStr, c_char};
use std::sync::{Arc, Mutex};

use amplifier_core::session::{Session, SessionConfig};

use crate::handles::{
    AmplifierHandle, AmplifierResult, AMPLIFIER_OK, ERR_INVALID_JSON, ERR_NULL_HANDLE, ERR_SESSION,
    arc_to_handle, handle_to_arc_owned, handle_to_arc_ref,
};
use crate::memory::{set_last_error, string_to_c};
use crate::runtime::FfiRuntime;

// ---------------------------------------------------------------------------
// Session wrapper
// ---------------------------------------------------------------------------

/// Wraps an `amplifier_core::Session` for FFI ownership transfer.
///
/// Holds a reference to the runtime that drives async execution and a
/// mutex-protected session for thread-safe FFI access.
pub struct FfiSession {
    /// The Tokio runtime used to drive async session operations.
    pub(crate) runtime: Arc<FfiRuntime>,
    /// The underlying session, mutex-protected for thread-safe FFI access.
    pub(crate) session: Mutex<Session>,
}

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Create a new session from a runtime handle and a JSON configuration string.
///
/// Parses `config_json` as a `SessionConfig`, creates a new `Session`, and
/// writes the session handle to `*out`.
///
/// Returns:
/// - `AMPLIFIER_OK` on success.
/// - `ERR_NULL_HANDLE` if any pointer argument is null.
/// - `ERR_INVALID_JSON` if `config_json` is not valid UTF-8 or valid session JSON.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_create(
    runtime: AmplifierHandle,
    config_json: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_session_create: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if config_json.is_null() {
        set_last_error("amplifier_session_create: config_json pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_session_create: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Borrow the runtime Arc — handle remains valid after this call.
    // SAFETY: handle was created by amplifier_runtime_create; non-null verified above.
    let runtime_arc = match unsafe { handle_to_arc_ref::<FfiRuntime>(runtime) } {
        Some(arc) => arc,
        None => {
            set_last_error("amplifier_session_create: invalid runtime handle");
            return ERR_NULL_HANDLE;
        }
    };

    // Parse config JSON string.
    // SAFETY: config_json is non-null (verified above); caller ensures valid C string.
    let config_str = match unsafe { CStr::from_ptr(config_json).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_session_create: config_json is not valid UTF-8");
            return ERR_INVALID_JSON;
        }
    };

    let session_config = match SessionConfig::from_json(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(&format!("amplifier_session_create: invalid config: {e}"));
            return ERR_INVALID_JSON;
        }
    };

    let session = Session::new(session_config, None, None);
    let ffi_session = Arc::new(FfiSession {
        runtime: runtime_arc,
        session: Mutex::new(session),
    });

    let handle = arc_to_handle(ffi_session);

    // SAFETY: `out` was verified non-null above.
    unsafe {
        *out = handle;
    }

    AMPLIFIER_OK
}

/// Destroy a session handle created by `amplifier_session_create`.
///
/// Consumes the Arc, dropping the session when the reference count reaches zero.
/// Returns `ERR_NULL_HANDLE` if `session` is null.
// SAFETY: `session` is verified non-null before use; handle was created by `amplifier_session_create`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_destroy(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_destroy: handle is null");
        return ERR_NULL_HANDLE;
    }

    // Consume the Arc; when it drops, the session is destroyed.
    // SAFETY: handle was created by amplifier_session_create via arc_to_handle::<FfiSession>.
    let _ = unsafe { handle_to_arc_owned::<FfiSession>(session) };

    AMPLIFIER_OK
}

/// Mark the session as initialized and ready for execution.
///
/// Acquires the session mutex and calls `set_initialized()` on the inner session.
///
/// Returns:
/// - `AMPLIFIER_OK` on success.
/// - `ERR_NULL_HANDLE` if `session` is null.
/// - `ERR_SESSION` if the mutex is poisoned.
// SAFETY: `session` is verified non-null before use.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_initialize(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_initialize: handle is null");
        return ERR_NULL_HANDLE;
    }

    // SAFETY: handle was created by amplifier_session_create; non-null verified above.
    let session_arc = match unsafe { handle_to_arc_ref::<FfiSession>(session) } {
        Some(arc) => arc,
        None => {
            set_last_error("amplifier_session_initialize: invalid handle");
            return ERR_NULL_HANDLE;
        }
    };

    let guard = match session_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!(
                "amplifier_session_initialize: mutex poisoned: {e}"
            ));
            return ERR_SESSION;
        }
    };

    guard.set_initialized();

    AMPLIFIER_OK
}

/// Execute a prompt using the session's mounted orchestrator.
///
/// Blocks the calling thread until the async execution completes.
/// On success, writes a heap-allocated C string pointer to `*out_json`.
/// The caller must free the returned string with `amplifier_string_free`.
///
/// Returns:
/// - `AMPLIFIER_OK` on success.
/// - `ERR_NULL_HANDLE` if `session`, `prompt`, or `out_json` is null.
/// - `ERR_INVALID_JSON` if `prompt` is not valid UTF-8.
/// - `ERR_SESSION` if execution fails or the mutex is poisoned.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_execute(
    session: AmplifierHandle,
    prompt: *const c_char,
    out_json: *mut *mut c_char,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_execute: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if prompt.is_null() {
        set_last_error("amplifier_session_execute: prompt pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out_json.is_null() {
        set_last_error("amplifier_session_execute: out_json pointer is null");
        return ERR_NULL_HANDLE;
    }

    // SAFETY: handle was created by amplifier_session_create; non-null verified above.
    let session_arc = match unsafe { handle_to_arc_ref::<FfiSession>(session) } {
        Some(arc) => arc,
        None => {
            set_last_error("amplifier_session_execute: invalid handle");
            return ERR_NULL_HANDLE;
        }
    };

    // SAFETY: prompt is non-null (verified above); caller ensures valid C string.
    let prompt_owned = match unsafe { CStr::from_ptr(prompt).to_str() } {
        Ok(s) => s.to_owned(),
        Err(_) => {
            set_last_error("amplifier_session_execute: prompt is not valid UTF-8");
            return ERR_INVALID_JSON;
        }
    };

    let mut guard = match session_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!("amplifier_session_execute: mutex poisoned: {e}"));
            return ERR_SESSION;
        }
    };

    let result = session_arc
        .runtime
        .runtime
        .block_on(guard.execute(&prompt_owned));

    match result {
        Ok(response) => {
            let ptr = string_to_c(&response);
            // SAFETY: out_json was verified non-null above.
            unsafe {
                *out_json = ptr;
            }
            AMPLIFIER_OK
        }
        Err(e) => {
            set_last_error(&format!("amplifier_session_execute: {e}"));
            ERR_SESSION
        }
    }
}

/// Run session cleanup, emitting `session:end` and releasing resources.
///
/// Blocks the calling thread until the async cleanup completes.
///
/// Returns:
/// - `AMPLIFIER_OK` on success.
/// - `ERR_NULL_HANDLE` if `session` is null.
/// - `ERR_SESSION` if the mutex is poisoned.
// SAFETY: `session` is verified non-null before use.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_cleanup(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_cleanup: handle is null");
        return ERR_NULL_HANDLE;
    }

    // SAFETY: handle was created by amplifier_session_create; non-null verified above.
    let session_arc = match unsafe { handle_to_arc_ref::<FfiSession>(session) } {
        Some(arc) => arc,
        None => {
            set_last_error("amplifier_session_cleanup: invalid handle");
            return ERR_NULL_HANDLE;
        }
    };

    let guard = match session_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!("amplifier_session_cleanup: mutex poisoned: {e}"));
            return ERR_SESSION;
        }
    };

    session_arc.runtime.runtime.block_on(guard.cleanup());

    AMPLIFIER_OK
}

// ---------------------------------------------------------------------------
// Tests (written first — TDD RED before GREEN)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{AMPLIFIER_OK, ERR_NULL_HANDLE};
    use crate::runtime::{amplifier_runtime_create, amplifier_runtime_destroy};
    use std::ffi::CString;
    use std::ptr;

    /// Minimal valid session config JSON.
    const MINIMAL_CONFIG: &str =
        r#"{"session":{"orchestrator":"loop-basic","context":"context-simple"}}"#;

    /// Helper: create a runtime handle, asserting success.
    fn create_runtime() -> AmplifierHandle {
        let mut handle: AmplifierHandle = ptr::null_mut();
        let result = amplifier_runtime_create(&mut handle as *mut AmplifierHandle);
        assert_eq!(result, AMPLIFIER_OK, "runtime_create should succeed");
        assert!(!handle.is_null(), "runtime handle must be non-null");
        handle
    }

    /// Helper: create a session handle from an existing runtime, asserting success.
    fn create_session(runtime: AmplifierHandle) -> AmplifierHandle {
        let config_cstr = CString::new(MINIMAL_CONFIG).unwrap();
        let mut handle: AmplifierHandle = ptr::null_mut();
        let result = amplifier_session_create(
            runtime,
            config_cstr.as_ptr(),
            &mut handle as *mut AmplifierHandle,
        );
        assert_eq!(result, AMPLIFIER_OK, "session_create should succeed");
        assert!(!handle.is_null(), "session handle must be non-null");
        handle
    }

    // -----------------------------------------------------------------------
    // TDD RED: session_create_destroy_roundtrip
    // Full lifecycle: create runtime → create session → destroy session → destroy runtime
    // -----------------------------------------------------------------------

    /// Full create runtime → create session → destroy session → destroy runtime roundtrip.
    #[test]
    fn session_create_destroy_roundtrip() {
        let runtime_handle = create_runtime();
        let session_handle = create_session(runtime_handle);

        // Destroy session first (before runtime)
        let result = amplifier_session_destroy(session_handle);
        assert_eq!(result, AMPLIFIER_OK, "session_destroy should succeed");

        // Destroy runtime last
        let result = amplifier_runtime_destroy(runtime_handle);
        assert_eq!(result, AMPLIFIER_OK, "runtime_destroy should succeed");
    }

    // -----------------------------------------------------------------------
    // TDD RED: session_null_args_return_error
    // All functions return ERR_NULL_HANDLE for null arguments.
    // -----------------------------------------------------------------------

    /// All session functions return ERR_NULL_HANDLE for null arguments.
    #[test]
    fn session_null_args_return_error() {
        let runtime_handle = create_runtime();
        let config_cstr = CString::new(MINIMAL_CONFIG).unwrap();
        let prompt_cstr = CString::new("hello").unwrap();
        let mut out: AmplifierHandle = ptr::null_mut();
        let mut out_json: *mut c_char = ptr::null_mut();

        // Create a real session to use for testing non-first-arg null cases.
        let session_handle = create_session(runtime_handle);

        // amplifier_session_create: null runtime
        let result = amplifier_session_create(ptr::null_mut(), config_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "null runtime → ERR_NULL_HANDLE");

        // amplifier_session_create: null config_json
        let result = amplifier_session_create(runtime_handle, ptr::null(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "null config_json → ERR_NULL_HANDLE");

        // amplifier_session_create: null out
        let result =
            amplifier_session_create(runtime_handle, config_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "null out → ERR_NULL_HANDLE");

        // amplifier_session_destroy: null handle
        let result = amplifier_session_destroy(ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "null handle → ERR_NULL_HANDLE");

        // amplifier_session_initialize: null handle
        let result = amplifier_session_initialize(ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "null handle → ERR_NULL_HANDLE");

        // amplifier_session_execute: null session
        let result =
            amplifier_session_execute(ptr::null_mut(), prompt_cstr.as_ptr(), &mut out_json);
        assert_eq!(result, ERR_NULL_HANDLE, "null session → ERR_NULL_HANDLE");

        // amplifier_session_execute: null prompt (real session handle passes first check)
        let result =
            amplifier_session_execute(session_handle, ptr::null(), &mut out_json);
        assert_eq!(result, ERR_NULL_HANDLE, "null prompt → ERR_NULL_HANDLE");

        // amplifier_session_execute: null out_json (real session and prompt pass first two checks)
        let result = amplifier_session_execute(session_handle, prompt_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "null out_json → ERR_NULL_HANDLE");

        // amplifier_session_cleanup: null handle
        let result = amplifier_session_cleanup(ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "null handle → ERR_NULL_HANDLE");

        // Cleanup
        let _ = amplifier_session_destroy(session_handle);
        let _ = amplifier_runtime_destroy(runtime_handle);
    }

    // -----------------------------------------------------------------------
    // Additional: session_initialize works on a valid session
    // -----------------------------------------------------------------------

    /// `amplifier_session_initialize` marks the session as initialized.
    #[test]
    fn session_initialize_succeeds() {
        let runtime_handle = create_runtime();
        let session_handle = create_session(runtime_handle);

        let result = amplifier_session_initialize(session_handle);
        assert_eq!(result, AMPLIFIER_OK, "initialize should succeed");

        // Cleanup
        let _ = amplifier_session_destroy(session_handle);
        let _ = amplifier_runtime_destroy(runtime_handle);
    }

    // -----------------------------------------------------------------------
    // Additional: invalid JSON returns ERR_INVALID_JSON
    // -----------------------------------------------------------------------

    /// Passing invalid JSON to `amplifier_session_create` returns `ERR_INVALID_JSON`.
    #[test]
    fn session_create_invalid_json_returns_error() {
        use crate::handles::ERR_INVALID_JSON;

        let runtime_handle = create_runtime();
        let bad_json = CString::new("not json at all").unwrap();
        let mut out: AmplifierHandle = ptr::null_mut();

        let result = amplifier_session_create(runtime_handle, bad_json.as_ptr(), &mut out);
        assert_eq!(result, ERR_INVALID_JSON, "invalid JSON → ERR_INVALID_JSON");
        assert!(out.is_null(), "out should remain null on error");

        let _ = amplifier_runtime_destroy(runtime_handle);
    }
}
