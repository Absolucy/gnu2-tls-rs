// SPDX-License-Identifier: MPL-2.0
//! Thread exit destructors through glibc.

use core::ffi::{c_int, c_void};

unsafe extern "C" {
	// crtbegin defines one per linked object. glibc uses it to find *this*
	// .so and keep it mapped until the destructors ran, so dlclose can't pull
	// the code out from under them
	static __dso_handle: u8;

	// glibc 2.18+
	fn __cxa_thread_atexit_impl(
		dtor: unsafe extern "C" fn(*mut c_void),
		obj: *mut c_void,
		dso_symbol: *const c_void,
	) -> c_int;
}

/// Runs `dtor(obj)` when the current thread exits.
///
/// # Safety
/// `obj` must stay valid until then, and `dtor` must be fine to call with it.
pub(crate) unsafe fn register(obj: *mut c_void, dtor: unsafe extern "C" fn(*mut c_void)) {
	// always returns 0, glibc aborts on out-of-memory instead
	// SAFETY: forwarded from the caller
	unsafe { __cxa_thread_atexit_impl(dtor, obj, (&raw const __dso_handle).cast()) };
}
