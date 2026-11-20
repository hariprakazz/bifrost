# bifrost

A compiler cache for C and C++, written in Rust. It wraps clang or gcc, hashes everything a compile depends on, and reuses the object file when nothing changed.

## Usage

    cargo build --release
    make CC="target/release/bifrost clang"
    target/release/bifrost --stats

Set `BIFROST_DIR` to move the cache (default `~/.cache/bifrost`) and `BIFROST_LOG=1` to see hit and miss lines.

## Status

Local cache only. Plain compiles with `-c` and `-o` are cached, including depfiles from `-MD` and `-MMD`. Everything else runs the real compiler untouched. Remote caching and trace analysis are not started.

## Numbers

Apple clang on a 10 core MacBook Air, median of several runs, builds using all cores:

| project | plain | cold cache | warm cache |
|---------|-------|-----------|-----------|
| Lua 5.4.7 (5 runs) | 0.46s | 0.61s | 0.31s |
| fmt 14ac0d5, tests on (3 runs) | 12.78s | 13.93s | 2.08s |

A cold build is slower than no cache, because every miss preprocesses the file twice, and the penalty shrinks as files get heavier. The saving shrinks on small files too: on libuv (one unscripted run) a warm build was only about 15% faster and a cold build about 76% slower. Reproduce the first two rows with `bench/lua.sh` and `bench/fmt.sh`.

## Correctness

Tests compare bifrost against the plain compiler byte for byte, covering hits, header changes, optimisation flags, depfiles, debug builds in different directories, and response files. CI runs them on Ubuntu with clang and gcc, and on macOS with clang. The unit tests call clang only.

## Limits

Warnings are not replayed on a hit, there is no cache size limit, and several cases are untested (precompiled headers, `__TIME__`, environment variables beyond the macOS deployment ones). The full list and the cache key are in [DESIGN.md](DESIGN.md).

## Stack

Rust and BLAKE3.
