//! Group 5 kernel service lifecycle for FFI.
//!
//! Provides scaffolded FFI functions for managing the kernel service:
//! `amplifier_kernel_service_start` and `amplifier_kernel_service_stop`.
//!
//! All functions validate arguments (null checks) and return `ERR_INTERNAL`
//! with TODO comments until kernel service integration is complete.

use std::ffi::c_char;

use crate::handles::{AmplifierHandle, AmplifierResult, ERR_INTERNAL, ERR_NULL_HANDLE};
use crate::memory::set_last_error;

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Start the kernel service for the given session, listening on `port`.
///
/// Validates that `session` and `out_token` are non-null.
///
/// On success, writes a service token handle into `*out_token` that can be
/// used to identify or stop the service later.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if `session` or `out_token` is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel service integration).
///
/// # TODO
///
/// Once the kernel service API is available, replace the `ERR_INTERNAL`
/// return with:
///   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
///   2. Lock `session_arc.session`.
///   3. Start a kernel service listener on `port`.
///   4. Write the service token Arc handle into `*out_token`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_kernel_service_start(
    session: AmplifierHandle,
    port: u16,
    out_token: *mut AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_kernel_service_start: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if out_token.is_null() {
        set_last_error("amplifier_kernel_service_start: out_token pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Suppress unused variable warning for port until implementation.
    let _ = port;

    // TODO: Obtain the FfiSession Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, start a gRPC listener on `port`, and write
    //       the resulting service token Arc handle into *out_token.
    set_last_error(
        "amplifier_kernel_service_start: kernel service integration not yet implemented",
    );
    ERR_INTERNAL
}

/// Stop the kernel service associated with `session`.
///
/// Validates that `session` is non-null.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if `session` is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel service integration).
///
/// # TODO
///
/// Once the kernel service API is available, replace the `ERR_INTERNAL`
/// return with:
///   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
///   2. Lock `session_arc.session`.
///   3. Stop the running kernel service listener.
// SAFETY: `session` is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_kernel_service_stop(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_kernel_service_stop: session handle is null");
        return ERR_NULL_HANDLE;
    }

    // TODO: Obtain the FfiSession Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, and stop the running kernel service listener.
    set_last_error("amplifier_kernel_service_stop: kernel service integration not yet implemented");
    ERR_INTERNAL
}

// Suppress unused import warning for c_char until implementation uses it.
#[allow(dead_code)]
const _: fn() = || {
    let _: *const c_char;
};

// ---------------------------------------------------------------------------
// Tests (written first — TDD RED before GREEN)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::ERR_NULL_HANDLE;
    use std::ptr;

    /// Both kernel_service functions return ERR_NULL_HANDLE for null arguments.
    #[test]
    fn kernel_service_null_args_return_error() {
        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let mut out_token: AmplifierHandle = ptr::null_mut();

        // amplifier_kernel_service_start: null session
        let result = amplifier_kernel_service_start(ptr::null_mut(), 8080, &mut out_token);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "kernel_service_start: null session → ERR_NULL_HANDLE"
        );

        // amplifier_kernel_service_start: null out_token
        let result = amplifier_kernel_service_start(fake_handle, 8080, ptr::null_mut());
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "kernel_service_start: null out_token → ERR_NULL_HANDLE"
        );

        // amplifier_kernel_service_stop: null session
        let result = amplifier_kernel_service_stop(ptr::null_mut());
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "kernel_service_stop: null session → ERR_NULL_HANDLE"
        );
    }

    /// Valid arguments pass null checks and reach the unimplemented body,
    /// returning ERR_INTERNAL (not ERR_NULL_HANDLE).
    #[test]
    fn kernel_service_valid_args_returns_internal() {
        use crate::handles::ERR_INTERNAL;

        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let mut out_token: AmplifierHandle = ptr::null_mut();

        let result = amplifier_kernel_service_start(fake_handle, 8080, &mut out_token);
        assert_eq!(
            result, ERR_INTERNAL,
            "kernel_service_start: valid args → ERR_INTERNAL (scaffold TODO)"
        );

        let result = amplifier_kernel_service_stop(fake_handle);
        assert_eq!(
            result, ERR_INTERNAL,
            "kernel_service_stop: valid args → ERR_INTERNAL (scaffold TODO)"
        );
    }
}
