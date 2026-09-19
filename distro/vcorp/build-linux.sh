#!/usr/bin/env bash
set -euo pipefail

SRC=/mnt/c/working/goose
DST="${HOME}/goose-vcorp-build"

rm -rf "${DST}"
mkdir -p "${DST}"

rsync -a --delete \
  --exclude target \
  --exclude .git \
  --exclude ui/desktop/node_modules \
  --exclude ui/desktop/out \
  --exclude documentation/node_modules \
  --exclude .hermit \
  "${SRC}/" "${DST}/"

echo "COPIED"
du -sh "${DST}"
cmake --version | head -n 1
gcc --version | head -n 1

# shellcheck source=/dev/null
source "${HOME}/.cargo/env"
cd "${DST}"
rustc --version

echo "START_BUILD"
cargo build --release -p goose-cli --bin goose
echo "BUILD_OK"
ls -lh target/release/goose
