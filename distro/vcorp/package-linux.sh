#!/usr/bin/env bash
set -eu

STAGE="${HOME}/goose-vcorp-package"
OUT="/mnt/c/working/goose/distro/vcorp/dist/linux-x86_64"

mkdir -p "${STAGE}"
mkdir -p "${OUT}"

cp -f /mnt/c/working/goose/distro/vcorp/init-config.yaml "${STAGE}/init-config.yaml"
cp -f /mnt/c/working/goose/distro/vcorp/README.md "${STAGE}/README.md"
chmod +x "${STAGE}/goose"
"${STAGE}/goose" --version

cp -f "${STAGE}/goose" "${OUT}/goose"
cp -f "${STAGE}/init-config.yaml" "${OUT}/init-config.yaml"
cp -f "${STAGE}/README.md" "${OUT}/README.md"

tar -C "${STAGE}" -czf "${OUT}/goose-vcorp-linux-x86_64.tar.gz" goose init-config.yaml README.md
ls -lh "${OUT}"
echo PACKAGE_OK
