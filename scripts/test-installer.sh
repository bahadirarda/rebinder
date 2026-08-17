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

printf '%s\n' "Unix installer acceptance passed."
