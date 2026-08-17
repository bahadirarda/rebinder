#!/bin/sh

set -eu

repository="bahadirarda/rebinder"
requested_version="${REBINDER_VERSION:-latest}"
install_dir="${REBINDER_INSTALL_DIR:-${XDG_BIN_HOME:-}}"
temporary_dir=""
binary_temporary=""
binary_backup=""
destination=""
activated=0
completed=0

say() {
  printf '%s\n' "rebinder: $*"
}

fail() {
  printf '%s\n' "rebinder: error: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Install the Rebinder native CLI from a verified GitHub Release.

Usage:
  install.sh [--version <tag>] [--to <directory>]

Options:
  --version <tag>    Install an exact tag, such as v0.20260817.1.
  --to <directory>  Install rebinder into this directory.
  -h, --help        Show this help message.

Environment:
  REBINDER_VERSION      Exact release tag or version.
  REBINDER_INSTALL_DIR  Destination directory.
  XDG_BIN_HOME          Fallback destination before $HOME/.local/bin.
EOF
}

cleanup() {
  if [ "$completed" -ne 1 ] && [ "$activated" -eq 1 ]; then
    rm -f -- "$destination"
    if [ -n "$binary_backup" ] && [ -f "$binary_backup" ]; then
      mv "$binary_backup" "$destination" || say "recovery binary remains at $binary_backup"
    fi
  fi
  if [ -n "$temporary_dir" ] && [ -d "$temporary_dir" ]; then
    rm -rf -- "$temporary_dir"
  fi
  if [ -n "$binary_temporary" ] && [ -f "$binary_temporary" ]; then
    rm -f -- "$binary_temporary"
  fi
}

trap cleanup EXIT
trap 'exit 130' HUP INT TERM

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || fail "--version requires a value"
      requested_version="$2"
      shift 2
      ;;
    --to)
      [ "$#" -ge 2 ] || fail "--to requires a directory"
      install_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

for command_name in curl grep awk tar uname mktemp cp mv rm mkdir chmod; do
  command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done

case "$(uname -s)" in
  Linux) platform="unknown-linux-gnu" ;;
  Darwin) platform="apple-darwin" ;;
  *) fail "this installer supports Linux and macOS; use install.ps1 on Windows" ;;
esac

case "$(uname -m)" in
  x86_64|amd64) architecture="x86_64" ;;
  arm64|aarch64) architecture="aarch64" ;;
  *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [ "$requested_version" = "latest" ]; then
  latest_url="$(
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
      --retry 3 --output /dev/null --write-out '%{url_effective}' \
      "https://github.com/$repository/releases/latest"
  )"
  release_tag="${latest_url##*/}"
else
  case "$requested_version" in
    v*) release_tag="$requested_version" ;;
    *) release_tag="v$requested_version" ;;
  esac
fi

printf '%s\n' "$release_tag" | grep -Eq '^v0\.[0-9]{8}\.(0|[1-9][0-9]*)$' \
  || fail "release version must match v0.YYYYMMDD.REVISION: $release_tag"

if [ -z "$install_dir" ]; then
  [ -n "${HOME:-}" ] || fail "HOME is not set; pass --to or REBINDER_INSTALL_DIR"
  install_dir="$HOME/.local/bin"
fi

target="$architecture-$platform"
archive="rebinder-$release_tag-$target.tar.gz"
download_root="https://github.com/$repository/releases/download/$release_tag"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/rebinder-install.XXXXXX")"

download() {
  name="$1"
  output="$2"
  if [ -n "${REBINDER_TEST_RELEASE_DIR:-}" ]; then
    cp "$REBINDER_TEST_RELEASE_DIR/$name" "$output"
  else
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error --retry 3 \
      --output "$output" "$download_root/$name"
  fi
}

say "downloading $release_tag for $target"
download "$archive" "$temporary_dir/$archive"
download "SHA256SUMS" "$temporary_dir/SHA256SUMS"

checksum="$(awk -v archive="$archive" '$2 == archive { print $1 }' "$temporary_dir/SHA256SUMS")"
printf '%s\n' "$checksum" | grep -Eq '^[a-f0-9]{64}$' \
  || fail "release checksum is missing or invalid for $archive"

say "verifying SHA-256 checksum"
if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$temporary_dir"
    printf '%s  %s\n' "$checksum" "$archive" | sha256sum --check --status
  ) || fail "checksum verification failed"
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "$temporary_dir"
    printf '%s  %s\n' "$checksum" "$archive" | shasum -a 256 --check --status
  ) || fail "checksum verification failed"
else
  fail "sha256sum or shasum is required to verify the release"
fi

tar -xzf "$temporary_dir/$archive" -C "$temporary_dir"
staging="rebinder-$release_tag-$target"
source_binary="$temporary_dir/$staging/rebinder"
source_metadata="$temporary_dir/$staging/release.json"
[ -f "$source_binary" ] || fail "release archive does not contain rebinder"
[ -f "$source_metadata" ] || fail "release archive does not contain release.json"
release_version="${release_tag#v}"
grep -Fq "\"name\": \"rebinder\"" "$source_metadata" \
  && grep -Fq "\"version\": \"$release_version\"" "$source_metadata" \
  && grep -Fq "\"tag\": \"$release_tag\"" "$source_metadata" \
  && grep -Fq "\"target\": \"$target\"" "$source_metadata" \
  || fail "release metadata does not match the requested artifact"

mkdir -p "$install_dir" || fail "cannot create install directory: $install_dir"
destination="$install_dir/rebinder"
[ ! -L "$destination" ] || fail "executable destination must not be a symbolic link"
if [ -e "$destination" ] && [ ! -f "$destination" ]; then
  fail "executable destination is not a regular file: $destination"
fi
binary_temporary="$install_dir/.rebinder.$$.tmp"
binary_backup="$install_dir/.rebinder.$$.backup"
[ ! -e "$binary_temporary" ] || fail "temporary executable path already exists"
[ ! -e "$binary_backup" ] || fail "backup executable path already exists"

if command -v install >/dev/null 2>&1; then
  install -m 0755 "$source_binary" "$binary_temporary"
else
  cp "$source_binary" "$binary_temporary"
  chmod 0755 "$binary_temporary"
fi
if [ -f "$destination" ]; then
  mv "$destination" "$binary_backup"
fi
activated=1
mv "$binary_temporary" "$destination"
binary_temporary=""

installed_version="$("$destination" --version)"
[ "$installed_version" = "rebinder $release_version" ] \
  || fail "installed executable reported an unexpected version: $installed_version"

completed=1
activated=0
if [ -n "$binary_backup" ] && [ -f "$binary_backup" ]; then
  rm -f -- "$binary_backup"
fi
say "installed $release_tag to $destination"
case ":${PATH:-}:" in
  *":$install_dir:"*) ;;
  *) say "add $install_dir to PATH before running rebinder" ;;
esac
