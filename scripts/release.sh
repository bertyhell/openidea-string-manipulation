#!/usr/bin/env bash
# Builds, signs and publishes a release of this plugin.
#
#   scripts/release.sh            tag v<version>, push the tag, create the GitHub release
#   scripts/release.sh --dry-run  build and sign the package only
#
# The version comes from plugin.toml. The signing key is read from 1Password
# (override with OP_KEY_REF) and passed to `openidea-plugin pack` through a
# pipe, so it is never written to disk.
set -euo pipefail

OP_KEY_REF="${OP_KEY_REF:-op://Private/openidea string manipulation plugin signing key bert/key}"
PUBLISHER="bertyhell"
REPO="bertyhell/openidea-string-manipulation"

dry_run=false
case "${1:-}" in
  "") ;;
  --dry-run) dry_run=true ;;
  *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
esac

die() { echo "error: $*" >&2; exit 1; }
step() { printf '\n==> %s\n' "$*"; }

cd "$(dirname "$0")/.."

for tool in op openidea-plugin cargo sha256sum unzip jq git; do
  command -v "$tool" >/dev/null || die "$tool is not installed"
done
$dry_run || command -v gh >/dev/null || die "gh is not installed"

field() { sed -n "s/^$1 = \"\(.*\)\"$/\1/p" "$2" | head -n1; }
id="$(field id plugin.toml)"
version="$(field version plugin.toml)"
notes="$(field change-notes plugin.toml)"
crate_version="$(field version Cargo.toml)"
tag="v$version"
package="$id-$version.oiplugin"

[[ -n "$id" && -n "$version" ]] || die "could not read id and version from plugin.toml"
[[ "$version" == "$crate_version" ]] || die "plugin.toml says $version but Cargo.toml says $crate_version"
[[ "$id" == "$PUBLISHER."* ]] || die "$id is not a $PUBLISHER.* plugin"

if ! $dry_run; then
  step "Checking git"
  [[ "$(git branch --show-current)" == main ]] || die "release from main"
  [[ -z "$(git status --porcelain)" ]] || die "the working tree has uncommitted changes"
  git fetch --quiet origin main --tags
  [[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || die "main is not the same as origin/main; push or pull first"
  ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || die "tag $tag already exists; bump the version in plugin.toml and Cargo.toml"
  gh auth status >/dev/null 2>&1 || die "gh is not logged in"
fi

step "Testing"
cargo test --quiet

step "Building $id $version"
openidea-plugin build

step "Signing with the key from 1Password"
key="$(op read --no-newline "$OP_KEY_REF")" || die "could not read $OP_KEY_REF"
rm -f "$package"
openidea-plugin pack . --publisher "$PUBLISHER" --key <(printf '%s' "$key") --out "$package"
unset key

sha256="$(sha256sum "$package" | cut -d' ' -f1)"
public_key="$(unzip -p "$package" SIGNATURE.json | jq -r .key)"
url="https://github.com/$REPO/releases/download/$tag/$package"

if $dry_run; then
  step "Dry run: nothing was tagged or published"
else
  step "Tagging $tag"
  git tag -a "$tag" -m "$id $version"
  git push origin "$tag"

  step "Creating the GitHub release"
  gh release create "$tag" "$package" --repo "$REPO" --verify-tag --title "$version" --notes "$notes"
fi

cat <<EOF

Package:    $package
SHA-256:    $sha256
Public key: $public_key

Entry for plugins/$id.toml in l0ng-ai/openidea-plugins:

[[versions]]
version = "$version"
package = "$url"
sha256 = "$sha256"
EOF
