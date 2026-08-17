#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
ARCH="${DEB_ARCH:-amd64}"
TARGET_DIR="${TARGET_DIR:-$ROOT_DIR/target/release}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
BINARY="$TARGET_DIR/sshnav"
PACKAGE_NAME="sshnav"
MAINTAINER="${DEB_MAINTAINER:-sshnav maintainers <maintainers@example.com>}"

if [[ -z "$VERSION" ]]; then
  echo "could not determine package version from Cargo.toml" >&2
  exit 1
fi

if [[ ! -x "$BINARY" ]]; then
  echo "missing release binary: $BINARY" >&2
  echo "run: cargo build --release --locked" >&2
  exit 1
fi

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

PKG_ROOT="$STAGING/${PACKAGE_NAME}_${VERSION}_${ARCH}"
mkdir -p "$PKG_ROOT/DEBIAN"

install -D -m 0755 "$BINARY" "$PKG_ROOT/usr/bin/sshnav"
install -D -m 0644 "$ROOT_DIR/README.md" "$PKG_ROOT/usr/share/doc/sshnav/README.md"
install -D -m 0644 "$ROOT_DIR/SECURITY.md" "$PKG_ROOT/usr/share/doc/sshnav/SECURITY.md"
install -D -m 0644 "$ROOT_DIR/docs/architecture.md" "$PKG_ROOT/usr/share/doc/sshnav/architecture.md"
install -D -m 0644 "$ROOT_DIR/LICENSE" "$PKG_ROOT/usr/share/doc/sshnav/copyright"

cat > "$PKG_ROOT/DEBIAN/control" <<CONTROL
Package: sshnav
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: $MAINTAINER
Depends: openssh-client
Suggests: mosh, rsync
Homepage: https://github.com/Vulcanite/sshnav
Description: Fast local SSH inventory navigator and launcher
 sshnav is a CLI-first SSH inventory, picker, launcher, and
 OpenSSH interop tool for Linux and macOS terminals.
CONTROL

mkdir -p "$DIST_DIR"
dpkg-deb --root-owner-group --build "$PKG_ROOT" "$DIST_DIR/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
