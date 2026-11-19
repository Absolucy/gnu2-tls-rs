// SPDX-License-Identifier: MPL-2.0
//! A cdylib for `examples/dlopen.rs` to load.

use gnu2_tls::gnu2_tls;
use std::{
	cell::Cell,
	hint::black_box,
	sync::atomic::{AtomicU32, Ordering},
};

static DROPS: AtomicU32 = AtomicU32::new(0);

struct CountDrops;

impl Drop for CountDrops {
	fn drop(&mut self) {
		DROPS.fetch_add(1, Ordering::Relaxed);
	}
}

#[gnu2_tls]
static COUNTER: Cell<u32> = Cell::new(0);

// starts at 100 so dlopen.rs can tell the TLS image was copied in
#[gnu2_tls]
static CONST_COUNTER: Cell<u32> = const { Cell::new(100) };

#[gnu2_tls]
static TRACKED: CountDrops = CountDrops;

#[gnu2_tls]
static BENCH_GNU2: Cell<u32> = Cell::new(0);

#[gnu2_tls]
static BENCH_GNU2_CONST: Cell<u32> = const { Cell::new(0) };

thread_local! {
	#[expect(clippy::missing_const_for_thread_local, reason = "lazy like ours, so the comparison is fair")]
	static BENCH_STD: Cell<u32> = Cell::new(0);
	static BENCH_STD_CONST: Cell<u32> = const { Cell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bump() -> u32 {
	COUNTER.with(|counter| {
		counter.set(counter.get() + 1);
		counter.get()
	})
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bump_const() -> u32 {
	CONST_COUNTER.with(|counter| {
		counter.set(counter.get() + 1);
		counter.get()
	})
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_address() -> usize {
	COUNTER.with(|counter| counter.as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_touch_tracked() {
	TRACKED.with(|_| ());
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_drops() -> u32 {
	DROPS.load(Ordering::Relaxed)
}

// never inlined, so every iteration looks the thread local up again instead
// of the compiler hoisting it out of the loop
#[inline(never)]
fn bump_gnu2() {
	BENCH_GNU2.with(|value| value.set(value.get().wrapping_add(1)));
}

#[inline(never)]
fn bump_std() {
	BENCH_STD.with(|value| value.set(value.get().wrapping_add(1)));
}

#[inline(never)]
fn bump_gnu2_const() {
	BENCH_GNU2_CONST.with(|value| value.set(value.get().wrapping_add(1)));
}

#[inline(never)]
fn bump_std_const() {
	BENCH_STD_CONST.with(|value| value.set(value.get().wrapping_add(1)));
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bench_gnu2(iterations: u32) -> u32 {
	for _ in 0..black_box(iterations) {
		bump_gnu2();
	}
	BENCH_GNU2.get()
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bench_std(iterations: u32) -> u32 {
	for _ in 0..black_box(iterations) {
		bump_std();
	}
	BENCH_STD.get()
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bench_gnu2_const(iterations: u32) -> u32 {
	for _ in 0..black_box(iterations) {
		bump_gnu2_const();
	}
	BENCH_GNU2_CONST.get()
}

#[unsafe(no_mangle)]
pub extern "C" fn fixture_bench_std_const(iterations: u32) -> u32 {
	for _ in 0..black_box(iterations) {
		bump_std_const();
	}
	BENCH_STD_CONST.get()
}
