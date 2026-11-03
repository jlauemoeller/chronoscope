#!/usr/bin/env bash
#
# Builds the .rpm and .deb packages from this checkout.
#
#   scripts/package.sh            for installing on this machine
#   scripts/package.sh <target>   for a target triple, as the release workflow does
#
# The published packages come from .github/workflows/release.yml, which builds
# on an older distro so that they run on more systems.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

for tool in cargo-deb cargo-generate-rpm; do
    if ! command -v "$tool" >/dev/null; then
        echo "$tool is not installed; run: cargo install --locked cargo-deb cargo-generate-rpm" >&2
        exit 1
    fi
done

target_args=()
out_dir=target
if [ $# -gt 0 ]; then
    target_args=(--target "$1")
    out_dir="target/$1"
fi

# RPM versions cannot contain `-`. A prerelease like 0.2.0-rc.1 becomes
# 0.2.0~rc.1, which sorts before 0.2.0. cargo-deb does the same on its own.
version="$(cargo pkgid | sed 's/.*[#@]//' | tr '-' '~')"

cargo build --release --locked "${target_args[@]}"
cargo deb --no-build "${target_args[@]}"
cargo generate-rpm "${target_args[@]}" --set-metadata "version = \"${version}\""

echo
echo "Packages for version ${version}:"
ls -1 "${out_dir}/generate-rpm/chronoscope-${version}-"*.rpm "${out_dir}/debian/chronoscope_${version}-"*.deb
