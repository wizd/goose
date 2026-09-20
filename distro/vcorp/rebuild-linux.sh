#!/usr/bin/env bash
set -euo pipefail

SRC=/mnt/c/working/goose
DST="${HOME}/goose-vcorp-build"

if [[ ! -d "${DST}" ]]; then
  exec "$(dirname "$0")/build-linux.sh"
fi

rsync -a --delete \
  --exclude target \
  --exclude .git \
  --exclude ui/desktop/node_modules \
  --exclude ui/desktop/out \
  --exclude documentation/node_modules \
  --exclude .hermit \
  "${SRC}/" "${DST}/"

# shellcheck source=/dev/null
source "${HOME}/.cargo/env"
cd "${DST}"
echo "START_BUILD"
cargo build --release -p goose-cli --bin goose
echo "BUILD_OK"
ls -lh target/release/goose
