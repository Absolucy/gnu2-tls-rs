# gnu2-tls

`thread_local!` but faster inside `.so` files on 32-bit linux :3

ok so the problem: in a shared library on i686, every `thread_local!` access calls `___tls_get_addr`, which goes and looks your module up in a per-thread table. every. single. time.

`#[gnu2_tls]` swaps that for a GNU2 TLS descriptor call instead. if glibc has room for your `.so` in its spare static TLS space, that call is basically two instructions that hand back a fixed offset. way cheaper

on anything that isn't 32-bit x86 linux with glibc, it just expands to plain `std::thread_local!`, so it's safe to use everywhere

## usage

```rust
use gnu2_tls::gnu2_tls;
use std::cell::Cell;

#[gnu2_tls]
static COUNT: Cell<u32> = const { Cell::new(0) };

COUNT.set(COUNT.get() + 1);
```

that's it. you get a `LocalKey` with the same methods as std's (`with`, `with_borrow_mut`, `get`, `set`, all of them). lazy init and `= const { ... }` work the same way they do with `thread_local!`, and values get dropped at thread exit

## things to know

- glibc's spare static TLS space is only 512 bytes by default, shared by every dlopen'd module. if it runs out you fall back to a slower path that's about as fast as `___tls_get_addr` (so no worse than before). bump it with `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=<bytes>`
- by default it's careful and treats all the C caller-saved registers as clobbered, bc older glibc's slow path trashes SSE/x87 registers. works on any glibc with TLS descriptors
- the `glibc-gnu2-tls-fix` feature drops those clobbers for a bit more speed. it also makes your library require glibc's `GLIBC_ABI_GNU2_TLS` symbol version, so it flat out refuses to load on a glibc without the fix instead of silently corrupting registers. that's glibc 2.43+, or 2.39 to 2.42 builds with the august 2025 branch updates (*not* plain 2.42.0). check yours with `objdump -T /path/to/libc.so.6 | grep GLIBC_ABI_GNU2_TLS`

## benchmarks

one thread bumping a `Cell<u32>` 100 million times from inside a dlopen'd `.so`, so every access is a real lookup:

| | `#[gnu2_tls]` | `std::thread_local!` |
|---|---|---|
| lazy init | 2.2 ns | 115 ns |
| `const { ... }` | 2.2 ns | 116 ns |

that's a Ryzen 9 5900X, glibc 2.44, rustc 1.100, under WSL2, with the `.so` fitting in the spare static TLS space. turning on `glibc-gnu2-tls-fix` gave the same numbers here, since a loop this tiny has nothing to keep in registers across the access

it's one machine and one microbenchmark, and std's number is specific to this setup, so run it on yours before trusting the ratio (see below)

## trying it out

there's an example that dlopens a test `.so`, checks the thread locals behave, and benches them against std:

```sh
cargo build --release --examples --target i686-unknown-linux-gnu
target/i686-unknown-linux-gnu/release/examples/dlopen
```

## license

MPL-2.0
