# bifrost

A compiler cache for C and C++, written in Rust. It wraps clang or gcc, hashes everything a compile depends on, and reuses the object file when nothing changed.

## Usage

    cargo build --release
    make CC="target/release/bifrost clang"
    target/release/bifrost --stats

Set `BIFROST_DIR` to move the cache (default `~/.cache/bifrost`) and `BIFROST_LOG=1` to see hit and miss lines.

## Status

Local cache only. Plain compiles with `-c` and `-o` are cached, including `-MD -MF` depfiles. Everything else runs the real compiler untouched. Remote caching and trace analysis are not started.

## Numbers

Lua 5.4.7, Apple clang, median of 5 runs, `make -j` on all cores:

| plain | cold cache | warm cache |
|-------|-----------|-----------|
| 0.46s | 0.61s | 0.31s |

A cold build is slower than no cache, because every miss preprocesses the file twice. Lua is a small project, so treat this as a first measurement. Reproduce it with `bench/lua.sh`.

## Correctness

Tests compare bifrost against the plain compiler byte for byte, covering hits, header changes, optimisation flags, depfiles, debug builds in different directories, and response files. CI runs them on Ubuntu with clang and gcc, and on macOS with clang. The unit tests call clang only.

## Limits

Warnings are not replayed on a hit, there is no cache size limit, and several cases are untested (precompiled headers, `__TIME__`, environment variables). The full list and the cache key are in [DESIGN.md](DESIGN.md).

## Stack

Rust and BLAKE3.
