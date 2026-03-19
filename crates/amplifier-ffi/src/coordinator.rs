//! Coordinator mount points for FFI.
//!
//! Provides scaffolded FFI functions for mounting modules onto a session's
//! Coordinator: `amplifier_session_mount_provider`, `amplifier_session_mount_tool`,
//! `amplifier_session_set_orchestrator`, and `amplifier_session_set_context`.
//!
//! All functions validate arguments (null checks, UTF-8 name parsing) and
//! return `ERR_INTERNAL` with TODO comments until kernel Coordinator API
//! integration is complete.

use std::ffi::{CStr, c_char};

use crate::handles::{AmplifierHandle, AmplifierResult, ERR_INTERNAL, ERR_NULL_HANDLE};
use crate::memory::set_last_error;

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Mount a provider onto a session by name.
///
/// Validates all arguments: `session`, `provider`, and `name` must be
/// non-null, and `name` must be valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
///
/// # TODO
///
/// Once `Session`/`Coordinator` expose a public `mount_provider` that accepts
/// an FFI-wrapped `Arc<dyn Provider>`, replace the `ERR_INTERNAL` return with:
///   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
///   2. Lock `session_arc.session`.
///   3. Access the `Coordinator` via `session.coordinator()`.
///   4. Call `coordinator.mount_provider(name_str, provider_arc)`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_mount_provider(
    session: AmplifierHandle,
    provider: AmplifierHandle,
    name: *const c_char,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_mount_provider: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if provider.is_null() {
        set_last_error("amplifier_session_mount_provider: provider handle is null");
        return ERR_NULL_HANDLE;
    }
    if name.is_null() {
        set_last_error("amplifier_session_mount_provider: name pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 name.
    // SAFETY: name is non-null (verified above); caller ensures valid C string.
    let _name_str = match unsafe { CStr::from_ptr(name).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_session_mount_provider: name is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Obtain session Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, access the Coordinator, and call
    //       coordinator.mount_provider(_name_str, provider_arc).
    //       Requires a Provider trait object constructed from the FFI handle.
    set_last_error("amplifier_session_mount_provider: kernel integration not yet implemented");
    ERR_INTERNAL
}

/// Mount a tool onto a session by name.
///
/// Validates all arguments: `session`, `tool`, and `name` must be
/// non-null, and `name` must be valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
///
/// # TODO
///
/// Once `Session`/`Coordinator` expose a public `mount_tool` that accepts
/// an FFI-wrapped `Arc<dyn Tool>`, replace the `ERR_INTERNAL` return with:
///   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
///   2. Lock `session_arc.session`.
///   3. Access the `Coordinator` via `session.coordinator()`.
///   4. Call `coordinator.mount_tool(name_str, tool_arc)`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_mount_tool(
    session: AmplifierHandle,
    tool: AmplifierHandle,
    name: *const c_char,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_mount_tool: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if tool.is_null() {
        set_last_error("amplifier_session_mount_tool: tool handle is null");
        return ERR_NULL_HANDLE;
    }
    if name.is_null() {
        set_last_error("amplifier_session_mount_tool: name pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 name.
    // SAFETY: name is non-null (verified above); caller ensures valid C string.
    let _name_str = match unsafe { CStr::from_ptr(name).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_session_mount_tool: name is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Obtain session Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, access the Coordinator, and call
    //       coordinator.mount_tool(_name_str, tool_arc).
    //       Requires a Tool trait object constructed from the FFI handle.
    set_last_error("amplifier_session_mount_tool: kernel integration not yet implemented");
    ERR_INTERNAL
}

/// Set the orchestrator module on a session (single slot).
///
/// Validates `session` and `orchestrator` are non-null.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
///
/// # TODO
///
/// Once `Session`/`Coordinator` expose a public `set_orchestrator` that accepts
/// an FFI-wrapped `Arc<dyn Orchestrator>`, replace the `ERR_INTERNAL` return with:
///   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
///   2. Lock `session_arc.session`.
///   3. Access the `Coordinator` via `session.coordinator()`.
///   4. Call `coordinator.set_orchestrator(orchestrator_arc)`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_set_orchestrator(
    session: AmplifierHandle,
    orchestrator: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_set_orchestrator: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if orchestrator.is_null() {
        set_last_error("amplifier_session_set_orchestrator: orchestrator handle is null");
        return ERR_NULL_HANDLE;
    }

    // TODO: Obtain session Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, access the Coordinator, and call
    //       coordinator.set_orchestrator(orchestrator_arc).
    //       Requires an Orchestrator trait object constructed from the FFI handle.
    set_last_error("amplifier_session_set_orchestrator: kernel integration not yet implemented");
    ERR_INTERNAL
}

/// Set the context manager module on a session (single slot).
///
/// Validates `session` and `context` are non-null.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
///
/// # TODO
///
/// Once `Session`/`Coordinator` expose a public `set_context` that accepts
/// an FFI-wrapped `Arc<dyn ContextManager>`, replace the `ERR_INTERNAL` return with:
///   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
///   2. Lock `session_arc.session`.
///   3. Access the `Coordinator` via `session.coordinator()`.
///   4. Call `coordinator.set_context(context_arc)`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_session_set_context(
    session: AmplifierHandle,
    context: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_set_context: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if context.is_null() {
        set_last_error("amplifier_session_set_context: context handle is null");
        return ERR_NULL_HANDLE;
    }

    // TODO: Obtain session Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, access the Coordinator, and call
    //       coordinator.set_context(context_arc).
    //       Requires a ContextManager trait object constructed from the FFI handle.
    set_last_error("amplifier_session_set_context: kernel integration not yet implemented");
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

    /// All 4 coordinator mount functions return ERR_NULL_HANDLE for null arguments.
    #[test]
    fn mount_null_args_return_error() {
        // Use a non-null fake pointer to pass the "first argument" check when
        // testing the second argument.  We never dereference it in the scaffold.
        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let name_cstr = CString::new("test-module").unwrap();

        // ---- amplifier_session_mount_provider ----

        // null session
        let result =
            amplifier_session_mount_provider(ptr::null_mut(), fake_handle, name_cstr.as_ptr());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_provider: null session → ERR_NULL_HANDLE");

        // null provider
        let result =
            amplifier_session_mount_provider(fake_handle, ptr::null_mut(), name_cstr.as_ptr());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_provider: null provider → ERR_NULL_HANDLE");

        // null name
        let result = amplifier_session_mount_provider(fake_handle, fake_handle, ptr::null());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_provider: null name → ERR_NULL_HANDLE");

        // ---- amplifier_session_mount_tool ----

        // null session
        let result =
            amplifier_session_mount_tool(ptr::null_mut(), fake_handle, name_cstr.as_ptr());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_tool: null session → ERR_NULL_HANDLE");

        // null tool
        let result =
            amplifier_session_mount_tool(fake_handle, ptr::null_mut(), name_cstr.as_ptr());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_tool: null tool → ERR_NULL_HANDLE");

        // null name
        let result = amplifier_session_mount_tool(fake_handle, fake_handle, ptr::null());
        assert_eq!(result, ERR_NULL_HANDLE, "mount_tool: null name → ERR_NULL_HANDLE");

        // ---- amplifier_session_set_orchestrator ----

        // null session
        let result = amplifier_session_set_orchestrator(ptr::null_mut(), fake_handle);
        assert_eq!(
            result,
            ERR_NULL_HANDLE,
            "set_orchestrator: null session → ERR_NULL_HANDLE"
        );

        // null orchestrator
        let result = amplifier_session_set_orchestrator(fake_handle, ptr::null_mut());
        assert_eq!(
            result,
            ERR_NULL_HANDLE,
            "set_orchestrator: null orchestrator → ERR_NULL_HANDLE"
        );

        // ---- amplifier_session_set_context ----

        // null session
        let result = amplifier_session_set_context(ptr::null_mut(), fake_handle);
        assert_eq!(
            result,
            ERR_NULL_HANDLE,
            "set_context: null session → ERR_NULL_HANDLE"
        );

        // null context
        let result = amplifier_session_set_context(fake_handle, ptr::null_mut());
        assert_eq!(
            result,
            ERR_NULL_HANDLE,
            "set_context: null context → ERR_NULL_HANDLE"
        );
    }

    /// Valid UTF-8 names are parsed without error (functions proceed past
    /// validation and return ERR_INTERNAL from the scaffolded TODO body).
    #[test]
    fn mount_valid_utf8_name_passes_validation() {
        use crate::handles::ERR_INTERNAL;

        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let name_cstr = CString::new("my-provider").unwrap();

        // Both mount functions with valid UTF-8 name should return ERR_INTERNAL
        // (not ERR_NULL_HANDLE), meaning they passed the null check and UTF-8
        // validation before reaching the unimplemented body.
        let result =
            amplifier_session_mount_provider(fake_handle, fake_handle, name_cstr.as_ptr());
        assert_eq!(
            result, ERR_INTERNAL,
            "mount_provider: valid args → ERR_INTERNAL (scaffold TODO)"
        );

        let name_cstr2 = CString::new("my-tool").unwrap();
        let result = amplifier_session_mount_tool(fake_handle, fake_handle, name_cstr2.as_ptr());
        assert_eq!(
            result, ERR_INTERNAL,
            "mount_tool: valid args → ERR_INTERNAL (scaffold TODO)"
        );
    }

    /// Single-slot functions (set_orchestrator, set_context) with valid args
    /// return ERR_INTERNAL from the scaffolded TODO body.
    #[test]
    fn set_single_slot_valid_args_returns_internal() {
        use crate::handles::ERR_INTERNAL;

        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;

        let result = amplifier_session_set_orchestrator(fake_handle, fake_handle);
        assert_eq!(
            result, ERR_INTERNAL,
            "set_orchestrator: valid args → ERR_INTERNAL (scaffold TODO)"
        );

        let result = amplifier_session_set_context(fake_handle, fake_handle);
        assert_eq!(
            result, ERR_INTERNAL,
            "set_context: valid args → ERR_INTERNAL (scaffold TODO)"
        );
    }
}
