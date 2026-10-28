# bifrost
A compiler cache for C and C++.

Bifrost is a Rust project I'm building to avoid recompiling the same code unnecessarily. The idea is to identify compilation inputs by their content, cache the resulting object files, and reuse them when the inputs haven't changed.

I'm also exploring remote caching and compiler trace analysis, with the goal of making builds faster and easier to debug.

How it works

The plan is to build Bifrost around four main parts:

Compiler wrapper: Intercept GCC and Clang invocations and parse their arguments.

Content hashing: Use BLAKE3 to identify compilation inputs and generate cache keys.

Local cache: Store compiled artifacts and reuse them when a matching cache entry exists.

Remote cache and traces: Explore sharing cached artifacts across machines and collecting compiler timing information.

Cache correctness is the main challenge. A cached object file is only useful if the inputs that affect compilation match.

Tech stack

Rust · BLAKE3 · Tokio · GCC/Clang · gRPC (tonic)

Status

Currently in the initial setup and design stage. The features above describe the intended direction, not a claim that everything is implemented.

What's next

Get the compiler wrapper working with GCC and Clang.

Figure out which compiler arguments and files need to be part of the cache key.

Get a basic local cache working and verify that cache hits produce the same output as a normal compilation.

Test what happens when headers, compiler flags, or source files change.

Run a few real projects through Bifrost and compare build times with caching enabled and disabled.

Look into remote caching once the local implementation is reliable.
