# Bifrost design notes

## Flow of one compile

1. Expand `@file` response files into plain arguments.
2. Parse the arguments. Only a plain compile is handled: `-c`, one source (`.c`, `.cc`, `.cpp`, `.cxx`) and an explicit `-o`.
3. Run the preprocessor (`-E`) with the same flags. Headers, macros and `-I` paths are all resolved in that output.
4. Build the cache key and look it up. On a hit, copy the object (and the depfile) to the requested paths.
5. On a miss, run the real compiler on the original arguments. Store the result only if it exited with 0.

Anything Bifrost does not understand or cannot do safely runs the real compiler unchanged and returns its exit code.

## What is in the cache key

BLAKE3 over length-prefixed parts, so `-O 2` and `-O2` cannot collide:

- the compiler's `--version` output
- every argument except the `-o` pair
- the `-o` path as well, when a depfile is requested (the depfile names its target). Without `-MF` the depfile path is the `-o` path with a `.d` extension
- the working directory, when any argument starts with `-g`
- `MACOSX_DEPLOYMENT_TARGET`, `IPHONEOS_DEPLOYMENT_TARGET` and `SDKROOT` from the environment
- the preprocessed source

The key is deliberately wider than strictly needed. A wider key costs extra misses, never wrong hits.

## Storage

Objects live at `<root>/<first two hex digits>/<key>`, with the depfile beside it as `<key>.dep`. The root is `BIFROST_DIR`, else `~/.cache/bifrost`. Writes and reads go through a temp file plus rename, so a crash cannot leave a half-written object that looks like a hit. Hits and misses are appended to `events.log` one line each, which is what `bifrost --stats` counts.

## What always bypasses the cache

- link steps, several sources, a missing `-o`
- `-M` and `-MM`, and `-MF` without `-MD` or `-MMD`
- flags that write extra files: `-ftime-trace`, `--serialize-diagnostics`, `-MJ`, `-save-temps`, `-fsave-optimization-record`
- flags that read files the preprocessor does not see: `-fprofile-use`, `-fprofile-sample-use`, `-fsanitize-ignorelist`
- coverage and profile output: `--coverage`, `-fprofile-arcs`, `-ftest-coverage`, `-fprofile-generate`
- response files that contain quotes, backslashes, a nested `@`, or cannot be read

## How correctness is checked

Differential tests run Bifrost and the plain compiler on the same input and compare object bytes, and depfile bytes where relevant. Several guards were also checked by breaking the code on purpose and confirming a test failed.

## Known limits

- A hit does not replay the compiler's warnings, because stderr is not stored.
- A miss preprocesses the file twice. On Lua 5.4.7 (Apple clang, 5 runs, medians) a cold build was about 30% slower than a plain build and a warm build about 33% faster. Compiling the preprocessed text instead changes `-g` objects and warning snippets, so it is not done.
- `-g` builds do not share entries across directories.
- The compiler is identified by its `--version` text only, not by a hash of the binary.
- Other environment variables are not part of the key. Ones that change include paths show up in the preprocessed text, but nothing else has been checked.
- `__DATE__` and `__TIME__` end up in the preprocessed text, so such files should never hit. This is not tested.
- Precompiled headers are not supported and not tested.
- There is no size limit or eviction. The cache only grows.
- Compiles that bypass the cache are not counted in `--stats`.
