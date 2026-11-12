#!/usr/bin/env bash
# Publish the workspace crates to crates.io in dependency order.
# Crates whose version is already on crates.io are skipped, so a release that
# failed halfway can simply be re-run.
# Requires CARGO_REGISTRY_TOKEN in the environment (unless --dry-run).
# Usage: scripts/publish.sh [--dry-run]
set -euo pipefail

# Dependency order: each crate only depends on crates listed before it.
crates=(process-fun-core process-fun-macro process-fun)

dry_run=()
[[ "${1:-}" == "--dry-run" ]] && dry_run=(--dry-run)

metadata="$(cargo metadata --no-deps --format-version 1)"

# Guard against the list above drifting from the workspace members
members="$(jq -r '.packages[].name' <<<"$metadata" | sort)"
if [[ "$members" != "$(printf '%s\n' "${crates[@]}" | sort)" ]]; then
    echo "error: crate list in $0 does not match workspace members:" >&2
    echo "$members" >&2
    exit 1
fi

# Path of a crate in the crates.io sparse index (names here are all >= 4 chars).
index_url() {
    local name="${1,,}"
    echo "https://index.crates.io/${name:0:2}/${name:2:2}/${name}"
}

is_published() {
    local name="$1" version="$2" body
    body="$(curl -fsSL "$(index_url "$name")" 2>/dev/null)" || return 1
    jq -e --arg v "$version" 'select(.vers == $v)' <<<"$body" >/dev/null
}

if [[ ${#dry_run[@]} -gt 0 ]]; then
    # A dry run of each crate separately would fail to resolve unpublished
    # versions of the other workspace crates; --workspace handles that.
    exec cargo publish --workspace --locked --dry-run
fi

for crate in "${crates[@]}"; do
    version="$(jq -r --arg n "$crate" '.packages[] | select(.name == $n) | .version' <<<"$metadata")"
    if is_published "$crate" "$version"; then
        echo "$crate $version is already on crates.io, skipping"
        continue
    fi
    echo "Publishing $crate $version"
    # cargo waits until the new version is available in the index before
    # returning, so the next crate can depend on it.
    cargo publish --locked -p "$crate"
done
