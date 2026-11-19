// SPDX-License-Identifier: MPL-2.0
//! The expansion behind `#[gnu2_tls]`.

/// Expands one `#[gnu2_tls]` item. Not public API.
// the helper items all get a `__gnu2_tls` prefix because the user's initializer
// is pasted in right next to them and items aren't hygienic. a plain `init`
// here would swallow a call to the user's own `init()`
#[doc(hidden)]
#[macro_export]
macro_rules! __declare {
	// has to come first, `const { ... }` also parses as an expr
	($(#[$attr:meta])* $vis:vis static $name:ident: $ty:ty = const $init:block $(;)?) => {
		// a const, like std's thread_local!, so the fn pointer inside
		// is known at every use site and the access gets inlined
		#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
		$(#[$attr])*
		$vis const $name: $crate::LocalKey<$ty> = {
			// .tdata, not .tbss, since the TLS image has to carry the
			// initial value
			#[unsafe(link_section = ".tdata.gnu2_tls")]
			static __GNU2_TLS_STORAGE: $crate::__private::EagerSlot<$ty> =
				$crate::__private::EagerSlot::new($init);

			$crate::__slot_fn!(__GNU2_TLS_STORAGE: $crate::__private::EagerSlot<$ty>);

			#[inline]
			fn __gnu2_tls_get(_preset: ::core::option::Option<&mut ::core::option::Option<$ty>>) -> *const $ty {
				// SAFETY: __gnu2_tls_slot() returns this thread's copy of __GNU2_TLS_STORAGE
				unsafe { (*__gnu2_tls_slot()).get() }
			}

			// SAFETY: __gnu2_tls_get() returns this thread's value, or null once destroyed
			unsafe { $crate::LocalKey::new(__gnu2_tls_get) }
		};

		#[cfg(not(all(target_arch = "x86", target_os = "linux", target_env = "gnu")))]
		::std::thread_local! {
			$(#[$attr])*
			$vis static $name: $ty = const $init;
		}
	};
	($(#[$attr:meta])* $vis:vis static $name:ident: $ty:ty = $init:expr $(;)?) => {
		#[cfg(all(target_arch = "x86", target_os = "linux", target_env = "gnu"))]
		$(#[$attr])*
		$vis const $name: $crate::LocalKey<$ty> = {
			#[unsafe(link_section = ".tbss.gnu2_tls")]
			static __GNU2_TLS_STORAGE: $crate::__private::Slot<$ty> = $crate::__private::Slot::new();

			$crate::__slot_fn!(__GNU2_TLS_STORAGE: $crate::__private::Slot<$ty>);

			#[inline]
			fn __gnu2_tls_init() -> $ty {
				$init
			}

			#[inline]
			fn __gnu2_tls_get(preset: ::core::option::Option<&mut ::core::option::Option<$ty>>) -> *const $ty {
				// SAFETY: __gnu2_tls_slot() returns this thread's copy of __GNU2_TLS_STORAGE
				unsafe { (*__gnu2_tls_slot()).get_or_init(preset, __gnu2_tls_init) }
			}

			// SAFETY: __gnu2_tls_get() returns this thread's value, or null once destroyed
			unsafe { $crate::LocalKey::new(__gnu2_tls_get) }
		};

		#[cfg(not(all(target_arch = "x86", target_os = "linux", target_env = "gnu")))]
		::std::thread_local! {
			$(#[$attr])*
			$vis static $name: $ty = $init;
		}
	};
	($($item:tt)*) => {
		::core::compile_error!("#[gnu2_tls] expects `static NAME: Type = init;`");
	};
}

// the clobber choice lives in its own macro because a cfg(feature) inside
// __declare's expansion would check the *user's* features, not ours

/// Defines `fn __gnu2_tls_slot()`, which returns this thread's copy of `$storage`. Not
/// public API.
#[cfg(not(feature = "glibc-gnu2-tls-fix"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __slot_fn {
	($storage:ident: $slot_ty:ty) => {
		#[inline]
		fn __gnu2_tls_slot() -> *const $slot_ty {
			let slot: *const $slot_ty;
			// SAFETY: this is the GNU2 TLS descriptor sequence, byte for
			// byte what LLVM emits with -enable-tlsdesc. ebx has to hold
			// the GOT, or ld/lld refuse to relax it in executables.
			// clobber_abi("C") because glibc < 2.40 trashes SSE/x87 regs
			// on the dynamic TLS slow path (BZ #31372), and ebx is a
			// clobber anyway, which also dodges BZ #32996 on 2.40/2.41
			unsafe {
				::core::arch::asm!(
					"calll 2f",
					"2:",
					"popl %ebx",
					"3:",
					"addl $_GLOBAL_OFFSET_TABLE_+(3b-2b), %ebx",
					"leal {storage}@TLSDESC(%ebx), %eax",
					"calll *{storage}@TLSCALL(%eax)",
					"addl %gs:0, %eax",
					storage = sym $storage,
					out("eax") slot,
					out("ebx") _,
					clobber_abi("C"),
					options(att_syntax, pure, nomem),
				);
			}
			slot
		}
	};
}

/// Defines `fn __gnu2_tls_slot()`, which returns this thread's copy of `$storage`. Not
/// public API.
#[cfg(feature = "glibc-gnu2-tls-fix")]
#[doc(hidden)]
#[macro_export]
macro_rules! __slot_fn {
	($storage:ident: $slot_ty:ty) => {
		// makes whatever links this need glibc's GLIBC_ABI_GNU2_TLS version,
		// so it refuses to load on a glibc without the fix below instead of
		// trashing registers at random
		#[used]
		// SAFETY: only the address is taken, the marker is never read
		static __GNU2_TLS_REQUIRE_GLIBC_ABI: &u8 = unsafe { &$crate::__private::GLIBC_ABI_GNU2_TLS };

		#[inline]
		fn __gnu2_tls_slot() -> *const $slot_ty {
			let slot: *const $slot_ty;
			// SAFETY: this is the GNU2 TLS descriptor sequence, byte for
			// byte what LLVM emits with -enable-tlsdesc. ebx has to hold
			// the GOT, or ld/lld refuse to relax it in executables.
			// no clobber_abi: with the BZ #31372 fix every glibc resolver
			// only touches eax and flags, and ebx is ours anyway (which
			// also dodges BZ #32996 on 2.40/2.41)
			unsafe {
				::core::arch::asm!(
					"calll 2f",
					"2:",
					"popl %ebx",
					"3:",
					"addl $_GLOBAL_OFFSET_TABLE_+(3b-2b), %ebx",
					"leal {storage}@TLSDESC(%ebx), %eax",
					"calll *{storage}@TLSCALL(%eax)",
					"addl %gs:0, %eax",
					storage = sym $storage,
					out("eax") slot,
					out("ebx") _,
					options(att_syntax, pure, nomem),
				);
			}
			slot
		}
	};
}
