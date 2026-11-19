// SPDX-License-Identifier: MPL-2.0
//! The key type `#[gnu2_tls]` declares, mirroring `std::thread::LocalKey`.

use core::{error::Error, fmt};

/// A thread local declared with [`#[gnu2_tls]`](crate::gnu2_tls).
///
/// Works like [`std::thread::LocalKey`]: each thread gets its own value,
/// built on first access (or at compile time for `const { ... }`) and dropped
/// when the thread exits.
pub struct LocalKey<T: 'static> {
	inner: fn(Option<&mut Option<T>>) -> *const T,
}

impl<T: 'static> LocalKey<T> {
	/// # Safety
	/// `inner` must return a pointer to the current thread's value, which
	/// lives in thread-local storage until the thread exits, or null once it
	/// was destroyed. It may take the preset value instead of running the
	/// initializer.
	#[doc(hidden)]
	#[must_use]
	pub const unsafe fn new(inner: fn(Option<&mut Option<T>>) -> *const T) -> Self {
		Self { inner }
	}

	/// Calls `f` with a reference to this thread's value, initializing it
	/// first if needed.
	///
	/// # Panics
	/// Panics if the value was already destroyed, or is being destroyed, on
	/// this thread. Also panics if the initializer or `f` panics.
	#[track_caller]
	pub fn with<F, R>(&'static self, f: F) -> R
	where
		F: FnOnce(&T) -> R,
	{
		match self.try_with(f) {
			Ok(result) => result,
			Err(err) => panic_access_error(err),
		}
	}

	/// Like [`with`](Self::with), but returns an error instead of panicking
	/// when the value was already destroyed on this thread.
	pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
	where
		F: FnOnce(&T) -> R,
	{
		// SAFETY: the reference only lives for the call to `f`, and the value
		// stays alive until the thread exits
		let value = unsafe { self.value_ptr(None).as_ref() }.ok_or(AccessError)?;
		Ok(f(value))
	}

	/// Initializes the value with `value` if this thread hasn't touched the
	/// key yet, skipping the declared initializer. `f` gets `value` back if it
	/// wasn't used.
	#[track_caller]
	pub(crate) fn initialize_with<F, R>(&'static self, value: T, f: F) -> R
	where
		F: FnOnce(Option<T>, &T) -> R,
	{
		let mut value = Some(value);
		// SAFETY: same as try_with
		let Some(current) = (unsafe { self.value_ptr(Some(&mut value)).as_ref() }) else {
			panic_access_error(AccessError)
		};
		f(value, current)
	}

	fn value_ptr(&'static self, preset: Option<&mut Option<T>>) -> *const T {
		(self.inner)(preset)
	}
}

impl<T: 'static> fmt::Debug for LocalKey<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LocalKey").finish_non_exhaustive()
	}
}

/// Returned by [`LocalKey::try_with`] when the value was already destroyed on
/// this thread.
#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AccessError;

impl fmt::Debug for AccessError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("AccessError")
	}
}

impl fmt::Display for AccessError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("already destroyed")
	}
}

impl Error for AccessError {}

#[cold]
#[inline(never)]
#[track_caller]
fn panic_access_error(err: AccessError) -> ! {
	panic!("cannot access a Thread Local Storage value during or after destruction: {err:?}")
}
