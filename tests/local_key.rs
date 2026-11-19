// SPDX-License-Identifier: MPL-2.0
// these run in an executable, where the linker rewrites the descriptor access
// into a plain offset from the thread pointer. examples/dlopen.rs covers the
// real descriptor call

use gnu2_tls::gnu2_tls;
use std::{
	cell::{Cell, RefCell},
	sync::atomic::{AtomicBool, AtomicU32, Ordering},
	thread,
};

#[test]
fn init_runs_once_per_thread() {
	static INITS: AtomicU32 = AtomicU32::new(0);

	// declared inside a fn on purpose, to cover that expansion
	#[gnu2_tls]
	static COUNT: Cell<u32> = {
		INITS.fetch_add(1, Ordering::Relaxed);
		Cell::new(0)
	};

	fn bump_three_times() -> u32 {
		for _ in 0..3 {
			COUNT.with(|count| count.set(count.get() + 1));
		}
		COUNT.get()
	}

	assert_eq!(bump_three_times(), 3);
	assert_eq!(thread::spawn(bump_three_times).join().unwrap(), 3);
	assert_eq!(bump_three_times(), 6);
	assert_eq!(INITS.load(Ordering::Relaxed), 2);
}

static DROPS: AtomicU32 = AtomicU32::new(0);

struct CountDrops;

impl Drop for CountDrops {
	fn drop(&mut self) {
		DROPS.fetch_add(1, Ordering::Relaxed);
	}
}

#[gnu2_tls]
static TRACKED: CountDrops = CountDrops;

#[test]
fn destructor_runs_at_thread_exit() {
	let before = DROPS.load(Ordering::Relaxed);
	thread::spawn(|| TRACKED.with(|_| ())).join().unwrap();
	assert_eq!(DROPS.load(Ordering::Relaxed) - before, 1);
}

static ACCESS_FAILED: AtomicBool = AtomicBool::new(false);

struct ReadsItselfOnDrop;

impl Drop for ReadsItselfOnDrop {
	fn drop(&mut self) {
		ACCESS_FAILED.store(SELF_READER.try_with(|_| ()).is_err(), Ordering::Relaxed);
	}
}

#[gnu2_tls]
static SELF_READER: ReadsItselfOnDrop = ReadsItselfOnDrop;

#[test]
fn access_during_destruction_fails() {
	thread::spawn(|| SELF_READER.with(|_| ())).join().unwrap();
	assert!(ACCESS_FAILED.load(Ordering::Relaxed));
}

static RECURSIVE_DROPS: AtomicU32 = AtomicU32::new(0);

struct CountRecursiveDrops;

impl Drop for CountRecursiveDrops {
	fn drop(&mut self) {
		RECURSIVE_DROPS.fetch_add(1, Ordering::Relaxed);
	}
}

thread_local! {
	static IN_INIT: Cell<bool> = const { Cell::new(false) };
}

#[gnu2_tls]
static RECURSIVE: CountRecursiveDrops = {
	if !IN_INIT.replace(true) {
		RECURSIVE.with(|_| ());
	}
	CountRecursiveDrops
};

#[test]
fn recursive_init_drops_each_value_once() {
	thread::spawn(|| RECURSIVE.with(|_| ())).join().unwrap();
	// the inner value when the outer init replaces it, then the outer at exit
	assert_eq!(RECURSIVE_DROPS.load(Ordering::Relaxed), 2);
}

#[test]
fn const_starts_from_initializer_on_each_thread() {
	// non-zero on purpose, so a value that didn't come from the TLS image
	// would show up as 0
	#[gnu2_tls]
	static VALUE: Cell<u32> = const { Cell::new(42) };

	assert_eq!(VALUE.get(), 42);
	VALUE.set(7);
	assert_eq!(thread::spawn(|| VALUE.replace(9)).join().unwrap(), 42);
	assert_eq!(VALUE.get(), 7);
}

static CONST_DROPS: AtomicU32 = AtomicU32::new(0);

struct CountConstDrops;

impl Drop for CountConstDrops {
	fn drop(&mut self) {
		CONST_DROPS.fetch_add(1, Ordering::Relaxed);
	}
}

#[gnu2_tls]
static CONST_TRACKED: CountConstDrops = const { CountConstDrops };

#[test]
fn const_destructor_runs_once_at_thread_exit() {
	thread::spawn(|| {
		CONST_TRACKED.with(|_| ());
		CONST_TRACKED.with(|_| ());
	})
	.join()
	.unwrap();
	assert_eq!(CONST_DROPS.load(Ordering::Relaxed), 1);
}

static CONST_ACCESS_FAILED: AtomicBool = AtomicBool::new(false);

struct ConstReadsItselfOnDrop;

impl Drop for ConstReadsItselfOnDrop {
	fn drop(&mut self) {
		CONST_ACCESS_FAILED.store(
			CONST_SELF_READER.try_with(|_| ()).is_err(),
			Ordering::Relaxed,
		);
	}
}

#[gnu2_tls]
static CONST_SELF_READER: ConstReadsItselfOnDrop = const { ConstReadsItselfOnDrop };

#[test]
fn const_access_during_destruction_fails() {
	thread::spawn(|| CONST_SELF_READER.with(|_| ()))
		.join()
		.unwrap();
	assert!(CONST_ACCESS_FAILED.load(Ordering::Relaxed));
}

// named like the helpers the expansion used to define, which shadowed these
fn init() -> Cell<u32> {
	Cell::new(7)
}

const fn get() -> Cell<u32> {
	Cell::new(8)
}

#[test]
fn initializer_can_call_fns_named_like_the_helpers() {
	#[gnu2_tls]
	static LAZY: Cell<u32> = init();

	#[gnu2_tls]
	static CONST: Cell<u32> = const { get() };

	assert_eq!(LAZY.get(), 7);
	assert_eq!(CONST.get(), 8);
}

#[test]
fn set_skips_the_initializer() {
	static INITS: AtomicU32 = AtomicU32::new(0);

	#[gnu2_tls]
	static VALUE: RefCell<u32> = {
		INITS.fetch_add(1, Ordering::Relaxed);
		RefCell::new(0)
	};

	thread::spawn(|| {
		VALUE.set(5);
		assert_eq!(VALUE.with_borrow(|value| *value), 5);
		VALUE.set(7);
		assert_eq!(VALUE.take(), 7);
	})
	.join()
	.unwrap();
	assert_eq!(INITS.load(Ordering::Relaxed), 0);
}
