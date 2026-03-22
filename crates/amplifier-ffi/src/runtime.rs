//! Runtime create/destroy for FFI.
//!
//! Provides `amplifier_runtime_create` and `amplifier_runtime_destroy` for
//! managing a Tokio multi-thread runtime across the FFI boundary.

use std::sync::Arc;

use crate::handles::{
    arc_to_handle, handle_to_arc_owned, AmplifierHandle, AmplifierResult, AMPLIFIER_OK,
    ERR_NULL_HANDLE, ERR_RUNTIME,
};
use crate::memory::set_last_error;

// ---------------------------------------------------------------------------
// Runtime wrapper
// ---------------------------------------------------------------------------

/// Wraps a Tokio multi-thread runtime for FFI ownership transfer.
pub struct FfiRuntime {
    /// Holds the runtime alive; dropped (and shut down) when the Arc is consumed.
    #[allow(dead_code)]
    pub(crate) runtime: tokio::runtime::Runtime,
}

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Create a new multi-thread Tokio runtime.
///
/// On success writes a non-null handle into `*out` and returns `AMPLIFIER_OK`.
/// Returns `ERR_NULL_HANDLE` if `out` is null.
/// Returns `ERR_RUNTIME` if the runtime cannot be created.
// SAFETY: `out` is verified non-null before the write; no pointer dereference occurs on null.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_runtime_create(out: *mut AmplifierHandle) -> AmplifierResult {
    if out.is_null() {
        set_last_error("amplifier_runtime_create: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(&format!(
                "amplifier_runtime_create: failed to create runtime: {e}"
            ));
            return ERR_RUNTIME;
        }
    };

    let ffi_rt = Arc::new(FfiRuntime { runtime: rt });
    let handle = arc_to_handle(ffi_rt);

    // SAFETY: `out` was verified non-null above.
    unsafe {
        *out = handle;
    }

    AMPLIFIER_OK
}

/// Destroy a runtime handle created by `amplifier_runtime_create`.
///
/// Consumes the Arc, which drops the Tokio runtime and shuts it down.
/// Returns `ERR_NULL_HANDLE` if `runtime` is null.
// SAFETY: `runtime` is verified non-null before use; handle was created by `amplifier_runtime_create`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_runtime_destroy(runtime: AmplifierHandle) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_runtime_destroy: handle is null");
        return ERR_NULL_HANDLE;
    }

    // Consume the Arc; when it drops, Tokio Runtime::drop shuts down the runtime.
    // SAFETY: handle was created by `amplifier_runtime_create` via `arc_to_handle::<FfiRuntime>`.
    // Null was checked above; drop the Arc to shut down the runtime.
    let _ = unsafe { handle_to_arc_owned::<FfiRuntime>(runtime) };

    AMPLIFIER_OK
}

// ---------------------------------------------------------------------------
// Tests (written first — TDD RED before GREEN)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{AMPLIFIER_OK, ERR_NULL_HANDLE};
    use std::ptr;

    /// Create then destroy succeeds; handle is non-null.
    #[test]
    fn runtime_create_destroy_roundtrip() {
        let mut handle: AmplifierHandle = ptr::null_mut();
        let result = amplifier_runtime_create(&mut handle as *mut AmplifierHandle);
        assert_eq!(result, AMPLIFIER_OK);
        assert!(!handle.is_null());

        let destroy_result = amplifier_runtime_destroy(handle);
        assert_eq!(destroy_result, AMPLIFIER_OK);
    }

    /// Passing a null `out` pointer returns ERR_NULL_HANDLE.
    #[test]
    fn runtime_null_out_returns_error() {
        let result = amplifier_runtime_create(ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE);
    }

    /// Passing a null handle to destroy returns ERR_NULL_HANDLE.
    #[test]
    fn runtime_destroy_null_returns_error() {
        let result = amplifier_runtime_destroy(ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE);
    }
}
