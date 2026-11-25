# bifrost

[![ci](https://github.com/hariprakazz/bifrost/actions/workflows/ci.yml/badge.svg)](https://github.com/hariprakazz/bifrost/actions/workflows/ci.yml)

Install with `cargo install --git https://github.com/hariprakazz/bifrost`.

A compiler cache for C and C++, written in Rust. It wraps clang or gcc, hashes everything a compile depends on, and reuses the object file when nothing changed. The rule it follows: fast when it knows, safe when it doesn't.

I built it with AI assistance and I can explain each design decision. The reasoning is in [DESIGN.md](DESIGN.md).

## Usage

    cargo build --release
    make CC="target/release/bifrost clang"
    target/release/bifrost --stats

Set `BIFROST_DIR` to move the cache (default `~/.cache/bifrost`), `BIFROST_MAX_MB` to change the size limit (default 5120), and `BIFROST_LOG=1` to see hit and miss lines.

## Status

Local cache only. Plain compiles with `-c` and `-o` are cached, including depfiles from `-MD` and `-MMD`. Everything else runs the real compiler untouched. Remote caching and trace analysis are not started.

## Numbers

Apple clang on a 10 core MacBook Air, median of several runs, builds using all cores:

| project | plain | cold cache | warm cache |
|---------|-------|-----------|-----------|
| Lua 5.4.7 (5 runs) | 0.46s | 0.61s | 0.31s |
| fmt 14ac0d5, tests on (3 runs) | 12.78s | 13.93s | 2.08s |

A cold build is slower than no cache, because every miss preprocesses the file twice, and the penalty shrinks as files get heavier. The saving shrinks on small files too: on libuv (one unscripted run) a warm build was only about 15% faster and a cold build about 76% slower. Reproduce the first two rows with `bench/lua.sh` and `bench/fmt.sh`.

Against ccache 4.14.3 on fmt, same session, median of 3 runs (`bench/compare.sh`):

| tool | cold cache | warm cache |
|------|-----------|-----------|
| plain compiler | 14.62s | no cache |
| bifrost | 16.33s | 2.34s |
| ccache | 14.41s | 1.32s |

ccache is faster on both, and on a warm build bifrost takes about 1.8 times as long. Bifrost starts the compiler twice on every run (`--version` and `-E`) before it can look anything up, and ccache's direct mode can skip the preprocessor on a hit. Bifrost has no such mode.

## Correctness

Tests compare bifrost against the plain compiler byte for byte, covering hits, header changes, optimisation flags, depfiles, debug builds in different directories, and response files. CI runs them on Ubuntu with clang and gcc, and on macOS with clang. The unit tests call clang only.

## Limits

A miss loses coloured diagnostics in a terminal. The cache trim walks the whole directory on every miss: one measurement on one machine is in DESIGN.md. Several cases are untested (`__DATE__`, GCC precompiled headers, GCC compiler identity, environment variables beyond the macOS deployment ones), and nobody but me has run it on their own project yet. The full list and the cache key are in [DESIGN.md](DESIGN.md).

## Stack

Rust and BLAKE3.
