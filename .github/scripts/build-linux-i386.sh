#!/usr/bin/env bash
set -euo pipefail

# Frontend is architecture-neutral; compile it with Node on the native runner.
cd "${GITHUB_WORKSPACE:?}"
(cd veiga-s3cc-splitter && npm run build)

# The complete WebKitGTK/AppIndicator stack is available natively on Debian i386.
# Docker -i is required for the here-document to reach the container.
docker run --rm -i --platform linux/386 \
  -v "$GITHUB_WORKSPACE:/work" -w /work \
  -e CARGO_TARGET_DIR=/work/target/i686-unknown-linux-gnu \
  -e CARGO_INCREMENTAL=0 \
  debian:bookworm bash -s <<'NATIVE_I386'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
arch="$(dpkg --print-architecture)"
printf 'Debian package architecture: %s\n' "$arch"
[ "$arch" = i386 ] || { echo 'Expected a native Debian i386 userland' >&2; exit 1; }
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates curl build-essential pkg-config file binutils \
  libatomic1 libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  libxdo-dev libssl-dev librsvg2-dev

# Debian bookworm i386 publishes libatomic here. Do not take the first match
# from /usr/lib: Docker/QEMU can expose x86_64 paths that will not load.
atomic=/usr/lib/i386-linux-gnu/libatomic.so.1
if [ ! -e "$atomic" ]; then
  echo 'Native libatomic path missing; extract a fresh Debian i386 package' >&2
  mkdir -p /tmp/native-atomic
  cd /tmp/native-atomic
  apt-get download libatomic1:i386
  deb="$(find . -maxdepth 1 -name '*.deb' -print -quit)"
  [ -n "$deb" ] || { echo 'Failed to download native libatomic1' >&2; exit 1; }
  dpkg-deb --extract "$deb" /tmp/native-atomic/extracted
  atomic=/tmp/native-atomic/extracted/usr/lib/i386-linux-gnu/libatomic.so.1
  cd /work
fi
[ -e "$atomic" ] || { echo "Missing i386 library: $atomic" >&2; exit 1; }
file "$atomic"
readelf -h "$atomic" | grep -Eq 'Class:[[:space:]]*ELF32'
readelf -h "$atomic" | grep -Eq 'Machine:[[:space:]]*Intel 80386'
export LD_LIBRARY_PATH="$(dirname "$atomic"):/usr/lib/i386-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
echo "Using verified 32-bit libatomic: $atomic"

# Do not use generic installer: uname can report host x86_64 kernel.
curl -fsSL --retry 3 \
  -o /tmp/rustup-init \
  https://static.rust-lang.org/rustup/dist/i686-unknown-linux-gnu/rustup-init
chmod +x /tmp/rustup-init
file /tmp/rustup-init
readelf -h /tmp/rustup-init | grep -Eq 'Class:[[:space:]]*ELF32'
ldd /tmp/rustup-init | tee /tmp/rustup-deps.txt
if grep -q 'not found' /tmp/rustup-deps.txt; then
  echo 'Unresolved i386 library for rustup-init' >&2
  exit 1
fi
/tmp/rustup-init -y --profile minimal \
  --default-host i686-unknown-linux-gnu --default-toolchain stable
. "$HOME/.cargo/env"
rustc -vV
cargo --version
[ "$(rustc -vV | sed -n 's/^host: //p')" = i686-unknown-linux-gnu ] || {
  echo 'Rust host must be i686-unknown-linux-gnu' >&2
  exit 1
}

cd /work
cargo build --locked --release \
  --manifest-path veiga-s3cc-splitter/src-tauri/Cargo.toml \
  --bin veiga-s3cc-splitter

binary=target/i686-unknown-linux-gnu/release/veiga-s3cc-splitter
test -s "$binary"
file "$binary"
readelf -h "$binary" | grep -Eq 'Class:[[:space:]]*ELF32'
readelf -h "$binary" | grep -Eq 'Machine:[[:space:]]*Intel 80386'
cp "$binary" "target/i686-unknown-linux-gnu/release/Veiga's S3CC Splitter"
chmod 755 "target/i686-unknown-linux-gnu/release/Veiga's S3CC Splitter"
NATIVE_I386

file "target/i686-unknown-linux-gnu/release/Veiga's S3CC Splitter"
