# Bifrost design notes

Bifrost sits in front of a C or C++ compiler. If it has seen the exact same compile before, it hands back the stored object file. If it has any doubt, it runs the real compiler. That fallback is the main safety rule. A missed hit only costs time, a wrong hit gives a wrong binary.

## Flow of one compile

1. Expand `@file` response files into plain arguments.
2. Parse the arguments. Only a plain compile is handled: `-c`, one source (`.c`, `.cc`, `.cpp`, `.cxx`) and an explicit `-o`.
3. Run the preprocessor (`-E`) with the same flags. Headers, macros and `-I` paths are all resolved in that output.
4. Build the cache key and look it up. A hit needs the object, the depfile and the stored stderr to all exist. On a hit, replay the stderr and copy the files to the requested paths.
5. On a miss, run the real compiler on the original arguments with stderr piped, so it can be printed and stored. Store the result only if the exit code was 0, then trim the cache.

Anything Bifrost does not understand or cannot do safely runs the real compiler unchanged and returns its exit code.

## What is in the cache key

BLAKE3 over length-prefixed parts, so `-O 2` and `-O2` cannot collide. For each part, what goes wrong without it:

- the compiler's `--version` output: a different compiler version makes a different object.
- when that output has an `InstalledDir:` line (clang does), the path, size and modification time of the file with the compiler's own name in that directory: on macOS `/usr/bin/clang` is a small shim, so the version text alone cannot see the real compiler change underneath it.
- every argument except the `-o` pair: flags such as `-O0` and `-O2` give the same preprocessed text but different objects.
- the `-o` path as well, when a depfile is requested: the depfile names its target. Without `-MF` the depfile path is the `-o` path with a `.d` extension.
- the working directory, when any argument starts with `-g`: debug info records the build directory.
- `MACOSX_DEPLOYMENT_TARGET`, `IPHONEOS_DEPLOYMENT_TARGET` and `SDKROOT`: they change the object but not the preprocessed text.
- the preprocessed source: this covers the source, every header, every macro and every `-I` path.
- response files need no part of their own, because they are expanded into arguments before the key is built.

The key is wider than strictly needed on purpose. A wider key costs extra misses, never wrong hits.

## Storage

Objects live at `<root>/<first two hex digits>/<key>`, with the depfile beside it as `<key>.dep` and the compiler's stderr as `<key>.err`. The root is `BIFROST_DIR`, else `~/.cache/bifrost`. Writes go through a temp file plus rename, so a crash cannot leave a half-written object that looks like a hit. A unit test checks that a reader never sees a partial object. A restored file gets the current modification time, because a copy that kept the cached file's old time made build tools see an object older than its source. Hits and misses are appended to `events.log`, one line each, which is what `bifrost --stats` counts.

## What always bypasses the cache

- link steps, several sources, a missing `-o`
- `-M` and `-MM`, and `-MF` without `-MD` or `-MMD`
- flags that write extra files: `-ftime-trace`, `--serialize-diagnostics`, `-MJ`, `-save-temps`, `-fsave-optimization-record`
- flags that read files the preprocessor does not see: `-fprofile-use`, `-fprofile-sample-use`, `-fsanitize-ignorelist`, `-fsanitize-blacklist`
- coverage and profile output: `--coverage`, `-fprofile-arcs`, `-ftest-coverage`, `-fprofile-generate`
- response files that contain quotes, backslashes, a nested `@`, or cannot be read

## How correctness is checked

Differential tests run Bifrost and the plain compiler on the same input and compare object bytes, and depfile bytes where relevant. Several guards were also checked by breaking the code on purpose and confirming a test failed. Every wrong hit found so far has a test that failed before the fix: optimisation levels sharing one key, the build directory leaking into debug objects, response file contents missing from the key, macOS environment variables, the depfile target name, restored files with an old modification time, warnings dropped on a hit, and partial reads.

## When a wrong hit is still possible

- The compiler binary changes but keeps the same size and modification time.
- GCC: it is expected not to print an `InstalledDir:` line, so only the `--version` text identifies it. No test uses a real GCC.
- An environment variable outside the three above changes the object without changing the preprocessed text.
- `__DATE__` is not tested. `__TIME__` is, and a file using it gets a new key whenever the second changes.
- GCC precompiled headers (`.gch`) and other uses of precompiled headers are untested. One clang `-include-pch` test gave the same bytes as a plain compile after the header changed.

## Why a cold build is slower

A miss preprocesses the file twice: once for the key and once inside the real compile. On Lua 5.4.7 (Apple clang, 5 runs, medians) a cold build was about 33% slower than a plain build and a warm build about 33% faster. Compiling the preprocessed text instead would save the second pass, but it changes `-g` objects and warning snippets, so it is not done.

## Known limits

- Compiler stderr is captured through a pipe on a miss, so clang drops coloured diagnostics there unless `-fcolor-diagnostics` is passed. Build tools already pipe the output, so builds are unaffected.
- `-g` builds do not share entries across directories.
- The compiler is identified by its `--version` text plus the size and modification time of the binary named in `InstalledDir:`, not by a hash of the binary. One test swaps a fake compiler for a file of a different size and checks that the key changes. Size and modification time are not tested separately. If the output has no `InstalledDir:` line, or no file with the compiler's name is in that directory, only the `--version` text is used.
- GCC is not covered by the binary check. A GCC compile is identified by its `--version` text alone, because GCC is expected not to print an `InstalledDir:` line. A GCC rebuild that keeps the same version text would not change the key.
- Other environment variables are not part of the key. Ones that change include paths show up in the preprocessed text, but nothing else has been checked.
- After a miss that stores something, the cache is trimmed to 90% of a size limit (5 GiB, or `BIFROST_MAX_MB`), deleting the least recently used files first. This walks the whole cache directory on every miss. One measurement on one MacBook Air, with a synthetic cache of 50,000 files of 1 KB: a miss took about 119 ms, against about 34 ms with an empty cache. With `BIFROST_MAX_MB=1` on a cache of that size, the first miss took about 2.2 s and deleted about 49,000 files, and later misses were back to 38 to 48 ms. Larger caches, real objects, a cold directory cache and other machines are not measured. Unit tests cover the ordering and the dotfile rule, and one differential test checks that a zero limit keeps nothing.
- Compiles that bypass the cache are not counted in `--stats`.
- Concurrent bifrost processes are only smoke tested. The parallel compile test cannot catch a store race, because every process looks up the cache before any of them writes.
- Nobody except the author has run Bifrost on a project of their own yet.
