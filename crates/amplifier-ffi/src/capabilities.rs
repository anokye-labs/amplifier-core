//! Group 6 capability registration and retrieval for FFI.
//!
//! Provides scaffolded FFI functions for managing session capabilities:
//! `amplifier_register_capability` and `amplifier_get_capability`.
//!
//! All functions validate arguments (null checks, UTF-8 name/value parsing)
//! and return `ERR_INTERNAL` with TODO comments until kernel capability
//! integration is complete.

use std::ffi::{CStr, c_char};

use crate::handles::{AmplifierHandle, AmplifierResult, ERR_INTERNAL, ERR_NULL_HANDLE};
use crate::memory::set_last_error;

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Register a named capability on the session with a JSON value.
///
/// Validates that `session`, `name`, and `value_json` are all non-null, and
/// that `name` and `value_json` are valid UTF-8.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel capability integration).
///
/// # TODO
///
/// Once the kernel capability API is available, replace the `ERR_INTERNAL`
/// return with:
///   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
///   2. Lock `session_arc.session`.
///   3. Parse `value_json` as a JSON value.
///   4. Call `session.register_capability(_name_str, _value_json)`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_register_capability(
    session: AmplifierHandle,
    name: *const c_char,
    value_json: *const c_char,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_register_capability: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if name.is_null() {
        set_last_error("amplifier_register_capability: name pointer is null");
        return ERR_NULL_HANDLE;
    }
    if value_json.is_null() {
        set_last_error("amplifier_register_capability: value_json pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 name.
    // SAFETY: name is non-null (verified above); caller ensures valid C string.
    let _name_str = match unsafe { CStr::from_ptr(name).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_register_capability: name is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // Validate UTF-8 value_json.
    // SAFETY: value_json is non-null (verified above); caller ensures valid C string.
    let _value_json_str = match unsafe { CStr::from_ptr(value_json).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_register_capability: value_json is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Obtain the FfiSession Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, parse value_json as a serde_json::Value, and
    //       call session.register_capability(_name_str, parsed_value).
    set_last_error("amplifier_register_capability: kernel capability integration not yet implemented");
    ERR_INTERNAL
}

/// Retrieve a named capability from the session as a JSON string.
///
/// Validates that `session`, `name`, and `out_json` are all non-null, and
/// that `name` is valid UTF-8.
///
/// On success, writes a heap-allocated C string pointer containing the
/// capability's JSON value into `*out_json`. The caller must free the
/// returned string with `amplifier_string_free`.
///
/// # Returns
///
/// - `ERR_NULL_HANDLE` if any argument is null.
/// - `ERR_INTERNAL` after argument validation (TODO: kernel capability integration).
///
/// # TODO
///
/// Once the kernel capability API is available, replace the `ERR_INTERNAL`
/// return with:
///   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
///   2. Lock `session_arc.session`.
///   3. Call `session.get_capability(_name_str)`.
///   4. Serialize the result to JSON and write a C string pointer into `*out_json`.
// SAFETY: each pointer argument is verified non-null before any dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn amplifier_get_capability(
    session: AmplifierHandle,
    name: *const c_char,
    out_json: *mut *mut c_char,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_get_capability: session handle is null");
        return ERR_NULL_HANDLE;
    }
    if name.is_null() {
        set_last_error("amplifier_get_capability: name pointer is null");
        return ERR_NULL_HANDLE;
    }
    if out_json.is_null() {
        set_last_error("amplifier_get_capability: out_json pointer is null");
        return ERR_NULL_HANDLE;
    }

    // Validate UTF-8 name.
    // SAFETY: name is non-null (verified above); caller ensures valid C string.
    let _name_str = match unsafe { CStr::from_ptr(name).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error("amplifier_get_capability: name is not valid UTF-8");
            return ERR_INTERNAL;
        }
    };

    // TODO: Obtain the FfiSession Arc via handle_to_arc_ref::<FfiSession>(session),
    //       lock session_arc.session, call session.get_capability(_name_str), serialize
    //       the result to JSON, and write a C string pointer into *out_json.
    set_last_error("amplifier_get_capability: kernel capability integration not yet implemented");
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

    /// Both capability functions return ERR_NULL_HANDLE for null arguments.
    #[test]
    fn capability_null_args_return_error() {
        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let name_cstr = CString::new("my-capability").unwrap();
        let value_json_cstr = CString::new("{\"key\":\"value\"}").unwrap();
        let mut out_json: *mut c_char = ptr::null_mut();

        // amplifier_register_capability: null session
        let result = amplifier_register_capability(
            ptr::null_mut(),
            name_cstr.as_ptr(),
            value_json_cstr.as_ptr(),
        );
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "register_capability: null session → ERR_NULL_HANDLE"
        );

        // amplifier_register_capability: null name
        let result =
            amplifier_register_capability(fake_handle, ptr::null(), value_json_cstr.as_ptr());
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "register_capability: null name → ERR_NULL_HANDLE"
        );

        // amplifier_register_capability: null value_json
        let result =
            amplifier_register_capability(fake_handle, name_cstr.as_ptr(), ptr::null());
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "register_capability: null value_json → ERR_NULL_HANDLE"
        );

        // amplifier_get_capability: null session
        let result =
            amplifier_get_capability(ptr::null_mut(), name_cstr.as_ptr(), &mut out_json);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "get_capability: null session → ERR_NULL_HANDLE"
        );

        // amplifier_get_capability: null name
        let result = amplifier_get_capability(fake_handle, ptr::null(), &mut out_json);
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "get_capability: null name → ERR_NULL_HANDLE"
        );

        // amplifier_get_capability: null out_json
        let result = amplifier_get_capability(fake_handle, name_cstr.as_ptr(), ptr::null_mut());
        assert_eq!(
            result, ERR_NULL_HANDLE,
            "get_capability: null out_json → ERR_NULL_HANDLE"
        );
    }

    /// Valid arguments pass null checks and reach the unimplemented body,
    /// returning ERR_INTERNAL (not ERR_NULL_HANDLE).
    #[test]
    fn capability_valid_args_returns_internal() {
        use crate::handles::ERR_INTERNAL;

        let fake_handle: *mut std::ffi::c_void = 1usize as *mut std::ffi::c_void;
        let name_cstr = CString::new("my-capability").unwrap();
        let value_json_cstr = CString::new("{\"key\":\"value\"}").unwrap();
        let mut out_json: *mut c_char = ptr::null_mut();

        let result = amplifier_register_capability(
            fake_handle,
            name_cstr.as_ptr(),
            value_json_cstr.as_ptr(),
        );
        assert_eq!(
            result, ERR_INTERNAL,
            "register_capability: valid args → ERR_INTERNAL (scaffold TODO)"
        );

        let result = amplifier_get_capability(fake_handle, name_cstr.as_ptr(), &mut out_json);
        assert_eq!(
            result, ERR_INTERNAL,
            "get_capability: valid args → ERR_INTERNAL (scaffold TODO)"
        );
    }
}
