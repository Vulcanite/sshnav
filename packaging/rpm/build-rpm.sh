#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
TARGET_DIR="${TARGET_DIR:-$ROOT_DIR/target/release}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
BINARY="$TARGET_DIR/sshnav"
PACKAGE_NAME="sshnav"

if [[ -z "$VERSION" ]]; then
  echo "could not determine package version from Cargo.toml" >&2
  exit 1
fi

if [[ ! -x "$BINARY" ]]; then
  echo "missing release binary: $BINARY" >&2
  echo "run: cargo build --release --locked" >&2
  exit 1
fi

if ! command -v rpmbuild >/dev/null 2>&1; then
  echo "rpmbuild is required to build an RPM package" >&2
  exit 1
fi

RPM_TOP="$(mktemp -d)"
trap 'rm -rf "$RPM_TOP"' EXIT

mkdir -p \
  "$RPM_TOP/BUILD" \
  "$RPM_TOP/BUILDROOT" \
  "$RPM_TOP/RPMS" \
  "$RPM_TOP/SOURCES" \
  "$RPM_TOP/SPECS" \
  "$RPM_TOP/SRPMS" \
  "$RPM_TOP/rpmdb" \
  "$RPM_TOP/tmp"

SPEC="$RPM_TOP/SPECS/sshnav.spec"
cat > "$SPEC" <<'SPEC'
Name: sshnav
Version: %{sshnav_version}
Release: 1%{?dist}
Summary: Fast local SSH inventory navigator and launcher
License: MIT
URL: https://github.com/Vulcanite/sshnav
Requires: openssh-clients

%description
sshnav is a CLI-first SSH inventory, picker, launcher, recorder, and
OpenSSH interop tool for Linux and macOS terminals.

%prep

%build

%install
install -D -m 0755 %{sshnav_binary} %{buildroot}/usr/bin/sshnav
install -D -m 0644 %{sshnav_readme} %{buildroot}/usr/share/doc/sshnav/README.md
install -D -m 0644 %{sshnav_security} %{buildroot}/usr/share/doc/sshnav/SECURITY.md
install -D -m 0644 %{sshnav_architecture} %{buildroot}/usr/share/doc/sshnav/architecture.md
install -D -m 0644 %{sshnav_license} %{buildroot}/usr/share/licenses/sshnav/LICENSE

%files
/usr/bin/sshnav
%doc /usr/share/doc/sshnav/README.md
%doc /usr/share/doc/sshnav/SECURITY.md
%doc /usr/share/doc/sshnav/architecture.md
%license /usr/share/licenses/sshnav/LICENSE

%changelog
* Wed Jul 08 2026 sshnav maintainers <maintainers@example.com> - %{sshnav_version}-1
- Package sshnav release %{sshnav_version}.
SPEC

mkdir -p "$DIST_DIR"
rpmbuild -bb "$SPEC" \
  --define "_topdir $RPM_TOP" \
  --define "_dbpath $RPM_TOP/rpmdb" \
  --define "_tmppath $RPM_TOP/tmp" \
  --define "sshnav_version $VERSION" \
  --define "sshnav_binary $BINARY" \
  --define "sshnav_readme $ROOT_DIR/README.md" \
  --define "sshnav_security $ROOT_DIR/SECURITY.md" \
  --define "sshnav_architecture $ROOT_DIR/docs/architecture.md" \
  --define "sshnav_license $ROOT_DIR/LICENSE"

find "$RPM_TOP/RPMS" -type f -name "${PACKAGE_NAME}-*.rpm" -exec cp {} "$DIST_DIR/" \;
