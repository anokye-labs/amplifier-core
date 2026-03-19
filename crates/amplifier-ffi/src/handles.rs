//! Handle types and Arc-based memory management for FFI.
//!
//! Provides opaque handle types for passing Rust Arc<T> across the FFI boundary,
//! along with result codes used by all FFI functions.

use std::ffi::c_void;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Opaque handle type
// ---------------------------------------------------------------------------

/// Opaque handle to a Rust-managed object.
pub type AmplifierHandle = *mut c_void;

/// FFI result code type.
pub type AmplifierResult = i32;

// ---------------------------------------------------------------------------
// Result constants
// ---------------------------------------------------------------------------

/// Operation succeeded.
pub const AMPLIFIER_OK: AmplifierResult = 0;

/// A null handle was passed where a valid handle was required.
pub const ERR_NULL_HANDLE: AmplifierResult = -1;

/// The provided JSON string was invalid or could not be parsed.
pub const ERR_INVALID_JSON: AmplifierResult = -2;

/// A Tokio runtime error occurred.
pub const ERR_RUNTIME: AmplifierResult = -3;

/// A session-level error occurred.
pub const ERR_SESSION: AmplifierResult = -4;

/// An unexpected internal error occurred.
pub const ERR_INTERNAL: AmplifierResult = -99;

// ---------------------------------------------------------------------------
// Handle conversion functions
// ---------------------------------------------------------------------------

/// Convert an `Arc<T>` into an opaque `AmplifierHandle`.
///
/// Increments the reference count (the Arc's ref is transferred into the handle).
/// The caller is responsible for eventually passing the handle to
/// `handle_to_arc_owned` to reclaim and drop the underlying value.
pub fn arc_to_handle<T>(arc: Arc<T>) -> AmplifierHandle {
    Arc::into_raw(arc) as *mut c_void
}

/// Borrow an `Arc<T>` from an `AmplifierHandle` without consuming it.
///
/// Increments the reference count by cloning the Arc. The handle remains valid
/// after this call. Returns `None` if the handle is null.
///
/// # Safety
///
/// The handle must have been created by `arc_to_handle::<T>` and must not have
/// been consumed by `handle_to_arc_owned`.
pub unsafe fn handle_to_arc_ref<T>(handle: AmplifierHandle) -> Option<Arc<T>> {
    if handle.is_null() {
        return None;
    }
    let ptr = handle as *const T;
    // Reconstruct Arc to clone it (increment refcount), then forget the
    // reconstructed Arc so the raw pointer remains valid.
    let arc = Arc::from_raw(ptr);
    let cloned = Arc::clone(&arc);
    std::mem::forget(arc);
    Some(cloned)
}

/// Consume an `AmplifierHandle` and return the underlying `Arc<T>`.
///
/// After this call the handle must not be used. Returns `None` if the handle
/// is null.
///
/// # Safety
///
/// The handle must have been created by `arc_to_handle::<T>`. Each handle must
/// be consumed exactly once.
pub unsafe fn handle_to_arc_owned<T>(handle: AmplifierHandle) -> Option<Arc<T>> {
    if handle.is_null() {
        return None;
    }
    let ptr = handle as *const T;
    Some(Arc::from_raw(ptr))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_values() {
        assert_eq!(AMPLIFIER_OK, 0);
        assert_eq!(ERR_NULL_HANDLE, -1);
        assert_eq!(ERR_INVALID_JSON, -2);
        assert_eq!(ERR_RUNTIME, -3);
        assert_eq!(ERR_SESSION, -4);
        assert_eq!(ERR_INTERNAL, -99);
    }

    #[test]
    fn test_arc_to_handle_not_null() {
        let arc = Arc::new(42u32);
        let handle = arc_to_handle(arc);
        assert!(!handle.is_null());
        // Cleanup: consume the handle to avoid a leak
        unsafe { handle_to_arc_owned::<u32>(handle) };
    }

    #[test]
    fn test_handle_to_arc_ref_increments_refcount() {
        let arc = Arc::new(100u32);
        let strong_before = Arc::strong_count(&arc);
        let handle = arc_to_handle(Arc::clone(&arc));
        // handle_to_arc_ref should clone (increment refcount)
        let cloned = unsafe { handle_to_arc_ref::<u32>(handle) };
        assert!(cloned.is_some());
        assert_eq!(Arc::strong_count(&arc), strong_before + 2); // original handle + clone
        // Cleanup
        drop(cloned);
        unsafe { handle_to_arc_owned::<u32>(handle) };
    }

    #[test]
    fn test_handle_to_arc_owned_consumes() {
        let arc = Arc::new(42u32);
        let handle = arc_to_handle(arc);
        let owned = unsafe { handle_to_arc_owned::<u32>(handle) };
        assert!(owned.is_some());
        assert_eq!(*owned.unwrap(), 42u32);
    }

    #[test]
    fn test_null_handle_returns_none() {
        let result = unsafe { handle_to_arc_ref::<u32>(std::ptr::null_mut()) };
        assert!(result.is_none());

        let result = unsafe { handle_to_arc_owned::<u32>(std::ptr::null_mut()) };
        assert!(result.is_none());
    }
}
