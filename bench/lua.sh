#!/bin/sh
set -e
root=$(pwd)
bin=${BIFROST_BIN:-$root/target/release/bifrost}
runs=${RUNS:-5}
jobs=$(getconf _NPROCESSORS_ONLN)
work=$(mktemp -d)
cd "$work"
curl -sSL -O https://www.lua.org/ftp/lua-5.4.7.tar.gz
tar xzf lua-5.4.7.tar.gz
cd lua-5.4.7

secs() {
  /usr/bin/time -p sh -c "make -j$jobs posix CC='$1' >/dev/null 2>&1" 2>&1 | awk '/^real/ {print $2}'
}

mid() {
  sort -n "$1" | sed -n "$(( (runs + 1) / 2 ))p"
}

for i in $(seq "$runs"); do
  make clean >/dev/null 2>&1
  export BIFROST_DIR=$(mktemp -d)
  secs "clang -std=gnu99" >> plain.txt
  make clean >/dev/null 2>&1
  secs "$bin clang -std=gnu99" >> cold.txt
  make clean >/dev/null 2>&1
  secs "$bin clang -std=gnu99" >> warm.txt
done

echo "lua 5.4.7, $runs runs, median seconds"
echo "plain $(mid plain.txt)  cold $(mid cold.txt)  warm $(mid warm.txt)"
