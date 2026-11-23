#!/bin/sh
set -e
root=$(pwd)
bin=${BIFROST_BIN:-$root/target/release/bifrost}
runs=${RUNS:-3}
command -v ccache >/dev/null || { echo "ccache not found"; exit 1; }
work=$(mktemp -d)
cd "$work"
git clone -q --depth 1 https://github.com/fmtlib/fmt.git
cd fmt
rev=$(git rev-parse --short HEAD)
opts="-G Ninja -DCMAKE_BUILD_TYPE=Release -DFMT_TEST=ON -DFMT_DOC=OFF"
cmake -S . -B plain $opts >/dev/null
cmake -S . -B bifrost $opts -DCMAKE_CXX_COMPILER_LAUNCHER="$bin" >/dev/null
cmake -S . -B ccache $opts -DCMAKE_CXX_COMPILER_LAUNCHER=ccache >/dev/null

secs() {
  /usr/bin/time -p sh -c "ninja -C $1 >/dev/null 2>&1" 2>&1 | awk '/^real/ {print $2}'
}

mid() {
  sort -n "$1" | sed -n "$(( (runs + 1) / 2 ))p"
}

for i in $(seq "$runs"); do
  export BIFROST_DIR=$(mktemp -d) CCACHE_DIR=$(mktemp -d)
  ninja -C plain -t clean >/dev/null
  secs plain >> plain.txt
  for tool in bifrost ccache; do
    ninja -C $tool -t clean >/dev/null
    secs $tool >> ${tool}_cold.txt
    ninja -C $tool -t clean >/dev/null
    secs $tool >> ${tool}_warm.txt
  done
done

echo "fmt $rev, $runs runs, median seconds"
echo "plain $(mid plain.txt)"
echo "bifrost cold $(mid bifrost_cold.txt) warm $(mid bifrost_warm.txt)"
echo "ccache  cold $(mid ccache_cold.txt) warm $(mid ccache_warm.txt)"
