// SPDX-License-Identifier: MPL-2.0
//! Thread locals that use GNU2 TLS descriptors.
//!
//! In a `.so`, a normal `thread_local!` on i686 calls `___tls_get_addr` on
//! every access, which looks the module up in the thread's table of TLS
//! blocks. `#[gnu2_tls]` makes the access a call through a TLS descriptor
//! instead. When glibc finds room for the `.so` in its spare static TLS space,
//! that call lands in a two-instruction function that just returns a fixed
//! offset.
//!
//! ```
//! use gnu2_tls::gnu2_tls;
//! use std::cell::RefCell;
//!
//! #[gnu2_tls]
//! static STATE: RefCell<Vec<u32>> = RefCell::new(Vec::new());
//!
//! STATE.with(|state| state.borrow_mut().push(1));
//! STATE.with_borrow_mut(|state| state.push(2));
//! ```
//!
//! [`LocalKey`] has the same methods as [`std::thread::LocalKey`], including
//! the `Cell` and `RefCell` shortcuts. Values are initialized lazily on first
//! access, and dropped at thread exit. The `static` turns into a `const`, just
//! like with `thread_local!`.
//!
//! Like std, `= const { ... }` skips the lazy init. The value is built at
//! compile time and lands in the TLS image, and if it doesn't need `Drop`, an
//! access is nothing but the descriptor call:
//!
//! ```
//! use gnu2_tls::gnu2_tls;
//! use std::cell::Cell;
//!
//! #[gnu2_tls]
//! static COUNT: Cell<u32> = const { Cell::new(0) };
//!
//! COUNT.set(COUNT.get() + 1);
//! ```
//!
//! # Platforms
//!
//! Only 32-bit x86 Linux with glibc gets descriptors (`target_arch = "x86"`,
//! `target_os = "linux"`, `target_env = "gnu"`), so `i686-unknown-linux-gnu`
//! and friends like `i586-unknown-linux-gnu`. On every other target the
//! attribute expands to plain `std::thread_local!`, and [`LocalKey`] and
//! [`AccessError`] are std's own types.
//!
//! # glibc notes
//!
//! - The spare static TLS space is shared by every dlopen'd module and is 512
//!   bytes by default. Once it runs out, accesses go through a slower resolver
//!   that is roughly as fast as `___tls_get_addr`. Raise it with
//!   `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=<bytes>`.
//! - By default the access treats all C caller-saved registers as clobbered,
//!   because the slow resolver doesn't preserve SSE/x87 registers without the
//!   fix for glibc BZ #31372. So this is safe on every glibc that has TLS
//!   descriptors, unlike LLVM's `-C llvm-args=-enable-tlsdesc`, which assumes
//!   it does.
//!
//! # The `glibc-gnu2-tls-fix` feature
//!
//! Drops those clobbers, so the compiler can keep values in registers across
//! an access. Without the glibc fix, SSE/x87 registers would come back
//! changed on the slow path, with no error. So the feature also makes the
//! output require glibc's `GLIBC_ABI_GNU2_TLS` symbol version, and the loader
//! refuses to load it on a glibc that doesn't have the fix.
//!
//! That version exists in glibc 2.43 and newer, and in 2.39 to 2.42 builds
//! that picked up the August 2025 release branch updates. It is *not* in a
//! plain 2.42.0. Check a system with
//! `objdump -T /path/to/libc.so.6 | grep GLIBC_ABI_GNU2_TLS`.

pub use gnu2_tls_macros::gnu2_tls;

mod declare;

#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
mod cell_methods;
#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
mod local_key;
#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
mod slot;
#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
mod thread_atexit;

#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
pub use local_key::{AccessError, LocalKey};

#[cfg(not(all(target_arch = "x86", target_os = "linux", target_env = "gnu")))]
pub use std::thread::{AccessError, LocalKey};

#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
#[doc(hidden)]
pub mod __private {
	pub use crate::slot::{EagerSlot, Slot};

	#[cfg(feature = "glibc-gnu2-tls-fix")]
	unsafe extern "C" {
		// a version node glibc defines once its TLS descriptor
		// resolvers save every register (BZ #33129). referencing it
		// adds a version-needs entry the loader checks
		#[link_name = "GLIBC_ABI_GNU2_TLS@GLIBC_ABI_GNU2_TLS"]
		pub static GLIBC_ABI_GNU2_TLS: u8;
	}
}
