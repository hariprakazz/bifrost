#!/bin/sh
set -e
root=$(pwd)
bin=${BIFROST_BIN:-$root/target/release/bifrost}
runs=${RUNS:-3}
work=$(mktemp -d)
cd "$work"
git clone -q --depth 1 https://github.com/fmtlib/fmt.git
cd fmt
rev=$(git rev-parse --short HEAD)
opts="-G Ninja -DCMAKE_BUILD_TYPE=Release -DFMT_TEST=ON -DFMT_DOC=OFF"
cmake -S . -B plain $opts >/dev/null
cmake -S . -B cached $opts -DCMAKE_CXX_COMPILER_LAUNCHER="$bin" >/dev/null

secs() {
  /usr/bin/time -p sh -c "ninja -C $1 >/dev/null 2>&1" 2>&1 | awk '/^real/ {print $2}'
}

mid() {
  sort -n "$1" | sed -n "$(( (runs + 1) / 2 ))p"
}

for i in $(seq "$runs"); do
  export BIFROST_DIR=$(mktemp -d)
  ninja -C plain -t clean >/dev/null
  secs plain >> plain.txt
  ninja -C cached -t clean >/dev/null
  secs cached >> cold.txt
  ninja -C cached -t clean >/dev/null
  secs cached >> warm.txt
done

echo "fmt $rev, $runs runs, median seconds"
echo "plain $(mid plain.txt)  cold $(mid cold.txt)  warm $(mid warm.txt)"
