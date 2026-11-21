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
- when that output has an `InstalledDir:` line (clang does), the path, size and modification time of the file with the compiler's own name in that directory. This is the real binary behind a shim such as `/usr/bin/clang` on macOS
- every argument except the `-o` pair
- the `-o` path as well, when a depfile is requested (the depfile names its target). Without `-MF` the depfile path is the `-o` path with a `.d` extension
- the working directory, when any argument starts with `-g`
- `MACOSX_DEPLOYMENT_TARGET`, `IPHONEOS_DEPLOYMENT_TARGET` and `SDKROOT` from the environment
- the preprocessed source

The key is deliberately wider than strictly needed. A wider key costs extra misses, never wrong hits.

## Storage

Objects live at `<root>/<first two hex digits>/<key>`, with the depfile beside it as `<key>.dep` and the compiler's stderr as `<key>.err`. The root is `BIFROST_DIR`, else `~/.cache/bifrost`. Writes and reads go through a temp file plus rename, so a crash cannot leave a half-written object that looks like a hit, and a unit test checks that a reader never sees a partial object. A restored file gets the current modification time, because a copy that kept the cached file's old time made build tools see an object older than its source. Hits and misses are appended to `events.log` one line each, which is what `bifrost --stats` counts.

## What always bypasses the cache

- link steps, several sources, a missing `-o`
- `-M` and `-MM`, and `-MF` without `-MD` or `-MMD`
- flags that write extra files: `-ftime-trace`, `--serialize-diagnostics`, `-MJ`, `-save-temps`, `-fsave-optimization-record`
- flags that read files the preprocessor does not see: `-fprofile-use`, `-fprofile-sample-use`, `-fsanitize-ignorelist`, `-fsanitize-blacklist`
- coverage and profile output: `--coverage`, `-fprofile-arcs`, `-ftest-coverage`, `-fprofile-generate`
- response files that contain quotes, backslashes, a nested `@`, or cannot be read

## How correctness is checked

Differential tests run Bifrost and the plain compiler on the same input and compare object bytes, and depfile bytes where relevant. Several guards were also checked by breaking the code on purpose and confirming a test failed.

## Known limits

- Compiler stderr is captured through a pipe on a miss, so clang drops coloured diagnostics there unless `-fcolor-diagnostics` is passed. Build tools already pipe the output, so builds are unaffected.
- A miss preprocesses the file twice. On Lua 5.4.7 (Apple clang, 5 runs, medians) a cold build was about 33% slower than a plain build and a warm build about 33% faster. Compiling the preprocessed text instead changes `-g` objects and warning snippets, so it is not done.
- `-g` builds do not share entries across directories.
- The compiler is identified by its `--version` text plus the size and modification time of the binary named in `InstalledDir:`, not by a hash of the binary. One test swaps a fake compiler for a file of a different size and checks that the key changes. A change that keeps both size and mtime is not caught, and size and mtime are not tested separately. If the output has no `InstalledDir:` line (GCC), or no file with the compiler's name is in that directory, only the `--version` text is used.
- Other environment variables are not part of the key. Ones that change include paths show up in the preprocessed text, but nothing else has been checked.
- `__TIME__` ends up in the preprocessed text, so a file using it gets a new key whenever the second changes, and a test checks this. `__DATE__` works the same way but is not tested.
- Precompiled headers are not handled specially. One clang test (`-include-pch`) gave the same bytes as a plain compile after the header changed. GCC `.gch` files and other PCH uses are untested.
- After a miss that stores something, the cache is trimmed to 90% of a size limit (5 GiB, or `BIFROST_MAX_MB`), deleting the least recently used files first. This walks the whole cache directory on every miss, which is untested at large sizes. Unit tests cover the ordering and the dotfile rule, and one differential test checks that a zero limit keeps nothing.
- Compiles that bypass the cache are not counted in `--stats`.
- Concurrent bifrost processes are only smoke tested. The parallel compile test cannot catch a store race, because every process looks up the cache before any of them writes.
