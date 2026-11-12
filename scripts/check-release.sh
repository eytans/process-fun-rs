#!/usr/bin/env bash
# Verify that the workspace is ready to release VERSION:
#   - every workspace crate has version VERSION
#   - every dependency between workspace crates requires exactly VERSION
#   - CHANGELOG.md has a "## [VERSION]" section
# Usage: scripts/check-release.sh 0.1.3
set -euo pipefail

version="${1:?usage: $0 <version>}"
version="${version#v}"
status=0

metadata="$(cargo metadata --no-deps --format-version 1)"
members="$(jq -r '.packages[].name' <<<"$metadata")"

while read -r name crate_version; do
    if [[ "$crate_version" != "$version" ]]; then
        echo "error: $name has version $crate_version, expected $version" >&2
        status=1
    fi
done < <(jq -r '.packages[] | "\(.name) \(.version)"' <<<"$metadata")

while read -r pkg dep req kind; do
    # Dev-dependencies are not part of the published crate
    [[ "$kind" == "dev" ]] && continue
    if grep -qx "$dep" <<<"$members" && [[ "$req" != "^$version" ]]; then
        echo "error: $pkg depends on $dep $req, expected ^$version" >&2
        status=1
    fi
done < <(jq -r '.packages[] | .name as $p | .dependencies[]
                | "\($p) \(.name) \(.req) \(.kind // "normal")"' <<<"$metadata")

if ! grep -qF "## [$version]" CHANGELOG.md; then
    echo "error: CHANGELOG.md has no \"## [$version]\" section" >&2
    status=1
fi

[[ $status -eq 0 ]] && echo "Workspace is ready to release $version"
exit $status
