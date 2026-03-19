//! Thread-local error string storage and C string utilities for FFI.
//!
//! Provides a thread-local last-error mechanism so C callers can retrieve
//! human-readable error messages after an FFI function returns a non-zero
//! result code.

use std::cell::RefCell;
use std::ffi::{CString, c_char};

// ---------------------------------------------------------------------------
// Thread-local last error storage
// ---------------------------------------------------------------------------

thread_local! {
    /// Stores the most recent error message for this thread.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Store `msg` as the last error for the current thread.
pub fn set_last_error(msg: &str) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = CString::new(msg).ok();
    });
}

/// Convert a Rust `&str` to a heap-allocated `*mut c_char`.
///
/// The caller is responsible for freeing the returned pointer via
/// [`amplifier_string_free`].  Returns a null pointer if the string contains
/// interior NUL bytes.
pub fn string_to_c(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Public C ABI functions
// ---------------------------------------------------------------------------

/// Return a pointer to the last error message set on the current thread.
///
/// The pointer is valid until the next FFI call on this thread that may set
/// a new error.  The caller must **not** free this pointer.  Returns null if
/// no error has been recorded.
#[no_mangle]
pub extern "C" fn amplifier_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|cs| cs.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

/// Free a `*mut c_char` that was allocated by any `amplifier_*` function.
///
/// Null-safe: calling with a null pointer is a no-op.
///
/// # Safety
///
/// `ptr` must be either null or a pointer previously returned by an
/// `amplifier_*` function (i.e., allocated via [`CString::into_raw`]).
/// Passing any other pointer is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn amplifier_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    drop(CString::from_raw(ptr));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_set_and_get_last_error() {
        set_last_error("something went wrong");
        let ptr = amplifier_last_error();
        assert!(!ptr.is_null());
        let msg = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(msg, "something went wrong");
    }

    #[test]
    fn test_amplifier_string_free_null_safe() {
        // Must not panic on null pointer
        unsafe { amplifier_string_free(std::ptr::null_mut()) };
    }

    #[test]
    fn test_string_to_c_roundtrip() {
        let ptr = string_to_c("hello ffi");
        assert!(!ptr.is_null());
        let result = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(result, "hello ffi");
        // Free the allocated string
        unsafe { amplifier_string_free(ptr) };
    }

    #[test]
    fn test_string_to_c_with_nul_returns_null() {
        // Strings with interior NUL bytes cannot be represented as C strings
        let ptr = string_to_c("bad\0string");
        assert!(ptr.is_null());
    }

    #[test]
    fn test_no_error_returns_null() {
        // Clear any prior error by setting LAST_ERROR to None explicitly
        LAST_ERROR.with(|cell| {
            *cell.borrow_mut() = None;
        });
        let ptr = amplifier_last_error();
        assert!(ptr.is_null());
    }
}
