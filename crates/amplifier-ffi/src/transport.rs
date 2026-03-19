//! Group 4 gRPC transport loaders for FFI.
//!
//! Provides scaffolded FFI functions for loading gRPC-backed modules:
//! `amplifier_load_grpc_provider`, `amplifier_load_grpc_tool`,
//! `amplifier_load_grpc_orchestrator`, `amplifier_load_grpc_hook`,
//! `amplifier_load_grpc_context`, and `amplifier_load_grpc_approval`.
//!
//! All functions validate arguments (null checks, UTF-8 endpoint parsing) and
//! return `ERR_INTERNAL` with TODO comments until gRPC transport integration
//! is complete.

use std::ffi::{CStr, c_char};

use crate::handles::{AmplifierHandle, AmplifierResult, ERR_INTERNAL, ERR_NULL_HANDLE};
use crate::memory::set_last_error;

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Load a gRPC-backed provider module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
/// `endpoint` is valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC provider transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` into a connection URI.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn Provider` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_provider(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_provider: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_provider: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_provider: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_provider: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct a Provider trait object,
    //       and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_provider: gRPC transport not yet implemented");
    ERR_INTERNAL
}

/// Load a gRPC-backed tool module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
/// `endpoint` is valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC tool transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` into a connection URI.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn Tool` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_tool(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_tool: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_tool: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_tool: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_tool: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct a Tool trait object,
    //       and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_tool: gRPC transport not yet implemented");
    ERR_INTERNAL
}

/// Load a gRPC-backed orchestrator module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, `session_id`, and `out` are all
/// non-null, and that `endpoint` and `session_id` are valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC orchestrator transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` and `session_id` into connection parameters.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn Orchestrator` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_orchestrator(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    session_id: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_orchestrator: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_orchestrator: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if session_id.is_null() {
        set_last_error("amplifier_load_grpc_orchestrator: session_id pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_orchestrator: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_orchestrator: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // Validate UTF-8 session_id.
    // SAFETY: session_id is non-null (verified above); caller ensures valid C string.
    let _session_id_str = match unsafe { CStr::from_ptr(session_id).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_orchestrator: session_id is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct an Orchestrator trait object
    //       using the session_id, and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_orchestrator: gRPC transport not yet implemented");
    ERR_INTERNAL
}

/// Load a gRPC-backed hook module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
/// `endpoint` is valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC hook transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` into a connection URI.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn Hook` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_hook(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_hook: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_hook: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_hook: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_hook: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct a Hook trait object,
    //       and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_hook: gRPC transport not yet implemented");
    ERR_INTERNAL
}

/// Load a gRPC-backed context module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
/// `endpoint` is valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC context transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` into a connection URI.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn ContextManager` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_context(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_context: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_context: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_context: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_context: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct a ContextManager trait object,
    //       and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_context: gRPC transport not yet implemented");
    ERR_INTERNAL
}

/// Load a gRPC-backed approval module from the given endpoint.
///
/// Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
/// `endpoint` is valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
///
/// # TODO
///
/// Once a gRPC approval transport adapter is available, replace the
/// `ERR_INTERNAL` return with:
///   1. Parse `endpoint` into a connection URI.
///   2. Establish a gRPC channel using the runtime's Tokio executor.
///   3. Construct a `dyn ApprovalHandler` from the remote gRPC service.
///   4. Write the resulting Arc handle into `*out`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_load_grpc_approval(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_load_grpc_approval: runtime handle is null");
        return ERR_NULL_HANDLE;
    }
    if endpoint.is_null() {
        set_last_error("amplifier_load_grpc_approval: endpoint pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out.is_null() {
        set_last_error("amplifier_load_grpc_approval: out pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 endpoint.
    // SAFETY: endpoint is non-null (verified above); caller ensures valid C string.
    let _endpoint_str = match unsafe { CStr::from_ptr(endpoint).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_load_grpc_approval: endpoint is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Connect to the gRPC endpoint, construct an ApprovalHandler trait object,
    //       and write the resulting Arc handle into *out.
    set_last_error("amplifier_load_grpc_approval: gRPC transport not yet implemented");
    ERR_INTERNAL
}

// ---------------------------------------------------------------------------
// Tests (written first — TDD RED before GREEN)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::ERR_NULL_HANDLE;
    use std::ffi::CString;
    use std::ptr;

    /// All 6 gRPC loader functions return ERR_NULL_HANDLE for null arguments.
    #[test]
    fn transport_null_args_return_error() {
        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let endpoint_cstr = CString::new("http://localhost:50051").unwrap();
        let session_id_cstr = CString::new("test-session-id").unwrap();
        let mut out: AmplifierHandle = ptr::null_mut();

        // ---- amplifier_load_grpc_provider ----

        // null runtime
        let result =
            amplifier_load_grpc_provider(ptr::null_mut(), endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_provider: null runtime → ERR_NULL_HANDLE");

        // null endpoint
        let result = amplifier_load_grpc_provider(fake_handle, ptr::null(), &mut out);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_provider: null endpoint → ERR_NULL_HANDLE"
        );

        // null out
        let result =
            amplifier_load_grpc_provider(fake_handle, endpoint_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_provider: null out → ERR_NULL_HANDLE");

        // ---- amplifier_load_grpc_tool ----

        // null runtime
        let result = amplifier_load_grpc_tool(ptr::null_mut(), endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_tool: null runtime → ERR_NULL_HANDLE");

        // null endpoint
        let result = amplifier_load_grpc_tool(fake_handle, ptr::null(), &mut out);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_tool: null endpoint → ERR_NULL_HANDLE"
        );

        // null out
        let result =
            amplifier_load_grpc_tool(fake_handle, endpoint_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_tool: null out → ERR_NULL_HANDLE");

        // ---- amplifier_load_grpc_orchestrator ----

        // null runtime
        let result = amplifier_load_grpc_orchestrator(
            ptr::null_mut(),
            endpoint_cstr.as_ptr(),
            session_id_cstr.as_ptr(),
            &mut out,
        );
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_orchestrator: null runtime → ERR_NULL_HANDLE"
        );

        // null endpoint
        let result = amplifier_load_grpc_orchestrator(
            fake_handle,
            ptr::null(),
            session_id_cstr.as_ptr(),
            &mut out,
        );
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_orchestrator: null endpoint → ERR_NULL_HANDLE"
        );

        // null session_id
        let result = amplifier_load_grpc_orchestrator(
            fake_handle,
            endpoint_cstr.as_ptr(),
            ptr::null(),
            &mut out,
        );
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_orchestrator: null session_id → ERR_NULL_HANDLE"
        );

        // null out
        let result = amplifier_load_grpc_orchestrator(
            fake_handle,
            endpoint_cstr.as_ptr(),
            session_id_cstr.as_ptr(),
            ptr::null_mut(),
        );
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_orchestrator: null out → ERR_NULL_HANDLE"
        );

        // ---- amplifier_load_grpc_hook ----

        // null runtime
        let result = amplifier_load_grpc_hook(ptr::null_mut(), endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_hook: null runtime → ERR_NULL_HANDLE");

        // null endpoint
        let result = amplifier_load_grpc_hook(fake_handle, ptr::null(), &mut out);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_hook: null endpoint → ERR_NULL_HANDLE"
        );

        // null out
        let result =
            amplifier_load_grpc_hook(fake_handle, endpoint_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_hook: null out → ERR_NULL_HANDLE");

        // ---- amplifier_load_grpc_context ----

        // null runtime
        let result =
            amplifier_load_grpc_context(ptr::null_mut(), endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_context: null runtime → ERR_NULL_HANDLE");

        // null endpoint
        let result = amplifier_load_grpc_context(fake_handle, ptr::null(), &mut out);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_context: null endpoint → ERR_NULL_HANDLE"
        );

        // null out
        let result =
            amplifier_load_grpc_context(fake_handle, endpoint_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_context: null out → ERR_NULL_HANDLE");

        // ---- amplifier_load_grpc_approval ----

        // null runtime
        let result =
            amplifier_load_grpc_approval(ptr::null_mut(), endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_approval: null runtime → ERR_NULL_HANDLE");

        // null endpoint
        let result = amplifier_load_grpc_approval(fake_handle, ptr::null(), &mut out);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "load_grpc_approval: null endpoint → ERR_NULL_HANDLE"
        );

        // null out
        let result =
            amplifier_load_grpc_approval(fake_handle, endpoint_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(result, ERR_NULL_HANDLE, "load_grpc_approval: null out → ERR_NULL_HANDLE");
    }

    /// Valid UTF-8 endpoints pass argument validation and reach the unimplemented body,
    /// returning ERR_INTERNAL (not ERR_NULL_HANDLE).
    #[test]
    fn transport_valid_args_returns_internal() {
        use crate::handles::ERR_INTERNAL;

        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let endpoint_cstr = CString::new("http://localhost:50051").unwrap();
        let session_id_cstr = CString::new("test-session-id").unwrap();
        let mut out: AmplifierHandle = ptr::null_mut();

        let result =
            amplifier_load_grpc_provider(fake_handle, endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_INTERNAL, "load_grpc_provider: valid args → ERR_INTERNAL (scaffold TODO)");

        let result = amplifier_load_grpc_tool(fake_handle, endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_INTERNAL, "load_grpc_tool: valid args → ERR_INTERNAL (scaffold TODO)");

        let result = amplifier_load_grpc_orchestrator(
            fake_handle, endpoint_cstr.as_ptr(), session_id_cstr.as_ptr(), &mut out,
        );
        assert_eq!(result, ERR_INTERNAL, "load_grpc_orchestrator: valid args → ERR_INTERNAL (scaffold TODO)");

        let result = amplifier_load_grpc_hook(fake_handle, endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_INTERNAL, "load_grpc_hook: valid args → ERR_INTERNAL (scaffold TODO)");

        let result = amplifier_load_grpc_context(fake_handle, endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_INTERNAL, "load_grpc_context: valid args → ERR_INTERNAL (scaffold TODO)");

        let result = amplifier_load_grpc_approval(fake_handle, endpoint_cstr.as_ptr(), &mut out);
        assert_eq!(result, ERR_INTERNAL, "load_grpc_approval: valid args → ERR_INTERNAL (scaffold TODO)");
    }
}
