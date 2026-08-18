#!/bin/sh

set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' "$repository_root/Cargo.toml")"
target="$(rustc -vV | awk '/^host:/ { print $2 }')"
case "$target" in
  *-linux-gnu|*-apple-darwin) ;;
  *) printf '%s\n' "installer test skipped for $target"; exit 0 ;;
esac

test_root="$(mktemp -d "${TMPDIR:-/tmp}/rebinder-installer-test.XXXXXX")"
cleanup() {
  rm -rf -- "$test_root"
}
trap cleanup EXIT HUP INT TERM

tag="v$version"
staging="rebinder-$tag-$target"
release_dir="$test_root/release"
install_dir="$test_root/bin"
mkdir -p "$release_dir/$staging"
cp "$repository_root/target/debug/rebinder" "$release_dir/$staging/rebinder"
cp "$repository_root/README.md" "$repository_root/LICENSE" "$release_dir/$staging/"
cat > "$release_dir/$staging/release.json" <<EOF
{
  "name": "rebinder",
  "version": "$version",
  "buildId": "$version+sha.000000000000",
  "tag": "$tag",
  "commit": "0000000000000000000000000000000000000000",
  "commitDate": "2026-08-17",
  "target": "$target"
}
EOF
(
  cd "$release_dir"
  tar -czf "$staging.tar.gz" "$staging"
  sha256sum "$staging.tar.gz" > SHA256SUMS
)

REBINDER_VERSION="$tag" \
REBINDER_INSTALL_DIR="$install_dir" \
REBINDER_TEST_RELEASE_DIR="$release_dir" \
  sh "$repository_root/site/install.sh" >/dev/null

reported="$($install_dir/rebinder --version)"
[ "$reported" = "rebinder $version" ] || {
  printf '%s\n' "unexpected installed version: $reported" >&2
  exit 1
}

mock_bin="$test_root/mock-bin"
portable_install_dir="$test_root/portable-bin"
real_sha256sum="$(command -v sha256sum)"
mkdir -p "$mock_bin"
cat > "$mock_bin/sha256sum" <<'EOF'
#!/bin/sh

if [ "$#" -ne 1 ]; then
  printf '%s\n' "usage: sha256sum [-bctwz] [files ...]" >&2
  exit 1
fi

case "$1" in
  -*)
    printf '%s\n' "usage: sha256sum [-bctwz] [files ...]" >&2
    exit 1
    ;;
esac

exec "$REBINDER_TEST_REAL_SHA256SUM" "$1"
EOF
chmod +x "$mock_bin/sha256sum"

PATH="$mock_bin:$PATH" \
REBINDER_TEST_REAL_SHA256SUM="$real_sha256sum" \
REBINDER_VERSION="$tag" \
REBINDER_INSTALL_DIR="$portable_install_dir" \
REBINDER_TEST_RELEASE_DIR="$release_dir" \
  sh "$repository_root/site/install.sh" >/dev/null

portable_reported="$($portable_install_dir/rebinder --version)"
[ "$portable_reported" = "rebinder $version" ] || {
  printf '%s\n' "unexpected portable-checksum installed version: $portable_reported" >&2
  exit 1
}

tampered_release_dir="$test_root/tampered-release"
tampered_install_dir="$test_root/tampered-bin"
tampered_log="$test_root/tampered.log"
cp -R "$release_dir" "$tampered_release_dir"
printf '%s' "tampered" >> "$tampered_release_dir/$staging.tar.gz"

set +e
REBINDER_VERSION="$tag" \
REBINDER_INSTALL_DIR="$tampered_install_dir" \
REBINDER_TEST_RELEASE_DIR="$tampered_release_dir" \
  sh "$repository_root/site/install.sh" >"$tampered_log" 2>&1
tampered_status=$?
set -e

[ "$tampered_status" -ne 0 ] || {
  printf '%s\n' "tampered archive unexpectedly passed checksum verification" >&2
  exit 1
}
grep -Fq "checksum verification failed" "$tampered_log" || {
  printf '%s\n' "tampered archive did not report a checksum failure" >&2
  exit 1
}

printf '%s\n' "Unix installer acceptance passed."
