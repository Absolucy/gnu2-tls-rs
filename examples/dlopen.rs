// SPDX-License-Identifier: MPL-2.0
//! Loads `libfixture.so` with dlopen, checks the thread locals behave, and
//! times them against std's `thread_local!`.
//!
//! ```sh
//! cargo build --release --examples --target i686-unknown-linux-gnu
//! target/i686-unknown-linux-gnu/release/examples/dlopen
//! ```

use std::{
	ffi::{CStr, CString, c_char, c_int, c_void},
	mem,
	os::unix::ffi::OsStrExt,
	thread,
	time::Instant,
};

const RTLD_NOW: c_int = 2;
const THREADS: u32 = 4;
const BUMPS: u32 = 1000;
const BENCH_ITERATIONS: u32 = 100_000_000;

unsafe extern "C" {
	fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
	fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
	fn dlerror() -> *mut c_char;
}

#[derive(Clone, Copy)]
struct Fixture {
	bump: extern "C" fn() -> u32,
	bump_const: extern "C" fn() -> u32,
	address: extern "C" fn() -> usize,
	touch_tracked: extern "C" fn(),
	drops: extern "C" fn() -> u32,
	bench_gnu2: extern "C" fn(u32) -> u32,
	bench_std: extern "C" fn(u32) -> u32,
	bench_gnu2_const: extern "C" fn(u32) -> u32,
	bench_std_const: extern "C" fn(u32) -> u32,
}

fn last_dl_error() -> String {
	// SAFETY: dlerror returns null or a valid C string
	let message = unsafe { dlerror() };
	if message.is_null() {
		return "unknown error".to_owned();
	}
	// SAFETY: checked for null above
	unsafe { CStr::from_ptr(message) }
		.to_string_lossy()
		.into_owned()
}

/// # Safety
/// `T` must be a fn pointer type matching the symbol's real signature.
unsafe fn symbol<T: Copy>(handle: *mut c_void, name: &CStr) -> T {
	// SAFETY: forwarded from the caller
	let address = unsafe { dlsym(handle, name.as_ptr()) };
	assert!(!address.is_null(), "dlsym {name:?}: {}", last_dl_error());
	// SAFETY: forwarded from the caller
	unsafe { mem::transmute_copy(&address) }
}

fn load() -> Fixture {
	let path = std::env::current_exe()
		.expect("no path to the current exe")
		.with_file_name("libfixture.so");
	let path = CString::new(path.as_os_str().as_bytes()).expect("path has a nul byte");
	// SAFETY: path is a valid C string
	let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
	assert!(!handle.is_null(), "dlopen {path:?}: {}", last_dl_error());
	// SAFETY: the signatures match examples/fixture.rs
	unsafe {
		Fixture {
			bump: symbol(handle, c"fixture_bump"),
			bump_const: symbol(handle, c"fixture_bump_const"),
			address: symbol(handle, c"fixture_address"),
			touch_tracked: symbol(handle, c"fixture_touch_tracked"),
			drops: symbol(handle, c"fixture_drops"),
			bench_gnu2: symbol(handle, c"fixture_bench_gnu2"),
			bench_std: symbol(handle, c"fixture_bench_std"),
			bench_gnu2_const: symbol(handle, c"fixture_bench_gnu2_const"),
			bench_std_const: symbol(handle, c"fixture_bench_std_const"),
		}
	}
}

fn check(fixture: Fixture) {
	let run_thread = move || {
		for expected in 1..=BUMPS {
			assert_eq!((fixture.bump)(), expected);
			// the const key starts at 100 on every thread
			assert_eq!((fixture.bump_const)(), 100 + expected);
		}
		(fixture.touch_tracked)();
		(fixture.address)()
	};
	let mut addresses: Vec<usize> = (0..THREADS)
		.map(|_| thread::spawn(run_thread))
		.collect::<Vec<_>>()
		.into_iter()
		.map(|handle| handle.join().expect("worker thread panicked"))
		.collect();
	addresses.push(run_thread());

	assert_eq!((fixture.drops)(), THREADS, "one drop per exited thread");
	addresses.sort_unstable();
	addresses.dedup();
	assert_eq!(
		addresses.len(),
		THREADS as usize + 1,
		"every thread gets its own value"
	);
	println!(
		"checks passed: {} threads, distinct addresses, {THREADS} drops",
		THREADS + 1
	);
}

fn bench(name: &str, run: extern "C" fn(u32) -> u32) {
	run(1000);
	let start = Instant::now();
	let result = run(BENCH_ITERATIONS);
	let elapsed = start.elapsed();
	let nanos_per_access = elapsed.as_secs_f64() * 1e9 / f64::from(BENCH_ITERATIONS);
	println!("{name}: {nanos_per_access:.2} ns/access ({result})");
}

fn main() {
	let fixture = load();
	check(fixture);
	bench("gnu2      ", fixture.bench_gnu2);
	bench("std       ", fixture.bench_std);
	bench("gnu2 const", fixture.bench_gnu2_const);
	bench("std const ", fixture.bench_std_const);
}
