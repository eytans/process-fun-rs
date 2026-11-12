#!/usr/bin/env bash
# Print the CHANGELOG.md section for VERSION (without its heading).
# Usage: scripts/release-notes.sh 0.1.3
set -euo pipefail

version="${1:?usage: $0 <version>}"
version="${version#v}"

awk -v heading="## [$version]" '
    index($0, heading) == 1 { found = 1; next }
    found && /^## \[/ { exit }
    found { print }
' CHANGELOG.md | sed -e '/./,$!d'
