// SPDX-License-Identifier: MPL-2.0
//! Per-thread storage, ported from std's `sys/thread_local/native/lazy.rs`
//! and `eager.rs`.

use crate::thread_atexit;
use core::{
	cell::{Cell, UnsafeCell},
	ffi::c_void,
	mem::{self, MaybeUninit},
	ptr,
};

// `.tbss` starts out zero-filled, so all-zero bytes have to mean "not
// initialized yet"
#[derive(Clone, Copy)]
#[repr(u8)]
enum State {
	Uninitialized = 0,
	Alive,
	Destroyed,
}

/// The storage behind one `#[gnu2_tls]` key. Only meant to be used by the
/// macro expansion.
#[doc(hidden)]
pub struct Slot<T> {
	value: UnsafeCell<MaybeUninit<T>>,
	state: Cell<State>,
}

// SAFETY: the static holding a slot sits in `.tbss` and is only ever reached
// through its TLS descriptor, so each thread only touches its own copy
unsafe impl<T> Sync for Slot<T> {}

impl<T> Slot<T> {
	#[must_use]
	#[expect(
		clippy::new_without_default,
		reason = "only the macro expansion builds these, in a static"
	)]
	pub const fn new() -> Self {
		Self {
			value: UnsafeCell::new(MaybeUninit::uninit()),
			state: Cell::new(State::Uninitialized),
		}
	}

	/// Returns the value, initializing it first if needed. `preset` is used
	/// instead of `init` when it holds a value. Returns null once the slot was
	/// destroyed.
	///
	/// # Safety
	/// `self` must be this thread's copy of the slot.
	#[inline]
	pub unsafe fn get_or_init(&self, preset: Option<&mut Option<T>>, init: fn() -> T) -> *const T {
		if let State::Alive = self.state.get() {
			return self.value.get().cast();
		}
		// SAFETY: forwarded from the caller
		unsafe { self.get_or_init_slow(preset, init) }
	}

	#[cold]
	unsafe fn get_or_init_slow(&self, preset: Option<&mut Option<T>>, init: fn() -> T) -> *const T {
		match self.state.get() {
			State::Uninitialized => {}
			State::Alive => return self.value.get().cast(),
			State::Destroyed => return ptr::null(),
		}

		let value = preset.and_then(Option::take).unwrap_or_else(init);
		// SAFETY: no `with` scope can be open here: init already returned and
		// the next scope only starts once we hand back the pointer. so even if
		// init recursed into this key, nobody holds a reference to the old value
		let mut old = unsafe { self.value.get().replace(MaybeUninit::new(value)) };
		match self.state.replace(State::Alive) {
			State::Uninitialized => {
				if mem::needs_drop::<T>() {
					// SAFETY: self is this thread's slot, which lives until the thread exits
					unsafe {
						thread_atexit::register(ptr::from_ref(self).cast_mut().cast(), destroy::<T>)
					};
				}
			}
			// init read this same key, so the inner call already registered the
			// destructor. just drop what it stored
			// SAFETY: Alive means the old value was initialized
			State::Alive => unsafe { old.assume_init_drop() },
			State::Destroyed => unreachable!("thread destructors ran during a thread local's init"),
		}
		self.value.get().cast()
	}
}

/// # Safety
/// Only called by glibc at thread exit, with the slot pointer passed to
/// `register`.
unsafe extern "C" fn destroy<T>(slot: *mut c_void) {
	// SAFETY: glibc hands back the pointer we registered
	let slot = unsafe { &*slot.cast::<Slot<T>>() };
	// registered exactly once, on the move to Alive, so the value is live.
	// flip to Destroyed first so the value's own Drop can't read itself
	slot.state.set(State::Destroyed);
	// SAFETY: see above
	unsafe { (*slot.value.get()).assume_init_drop() };
}

/// The storage behind a `const { ... }` key. The value is already there from
/// the TLS image, so the only state is whether the destructor is registered.
#[doc(hidden)]
pub struct EagerSlot<T> {
	value: UnsafeCell<T>,
	// Uninitialized here just means "destructor not registered yet"
	state: Cell<State>,
}

// SAFETY: same as Slot, only reached through its TLS descriptor
unsafe impl<T> Sync for EagerSlot<T> {}

impl<T> EagerSlot<T> {
	#[must_use]
	pub const fn new(value: T) -> Self {
		Self {
			value: UnsafeCell::new(value),
			state: Cell::new(State::Uninitialized),
		}
	}

	/// Returns the value, or null once it was destroyed.
	///
	/// # Safety
	/// `self` must be this thread's copy of the slot.
	#[inline]
	pub unsafe fn get(&self) -> *const T {
		// nothing to drop means nothing to register and nothing to destroy,
		// so the access is just the descriptor call
		if !mem::needs_drop::<T>() {
			return self.value.get();
		}
		if let State::Alive = self.state.get() {
			return self.value.get();
		}
		// SAFETY: forwarded from the caller
		unsafe { self.register_slow() }
	}

	#[cold]
	unsafe fn register_slow(&self) -> *const T {
		match self.state.get() {
			State::Uninitialized => {}
			State::Alive => return self.value.get(),
			State::Destroyed => return ptr::null(),
		}
		// SAFETY: self is this thread's slot, which lives until the thread exits
		unsafe {
			thread_atexit::register(ptr::from_ref(self).cast_mut().cast(), destroy_eager::<T>)
		};
		self.state.set(State::Alive);
		self.value.get()
	}
}

/// # Safety
/// Only called by glibc at thread exit, with the slot pointer passed to
/// `register`.
unsafe extern "C" fn destroy_eager<T>(slot: *mut c_void) {
	// SAFETY: glibc hands back the pointer we registered
	let slot = unsafe { &*slot.cast::<EagerSlot<T>>() };
	// flip first so the value's own Drop can't read itself
	slot.state.set(State::Destroyed);
	// SAFETY: registered once, on the move to Alive, so this is the only drop
	unsafe { ptr::drop_in_place(slot.value.get()) };
}
