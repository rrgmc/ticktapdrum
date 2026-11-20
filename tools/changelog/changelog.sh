#!/usr/bin/env bash
# Works with CHANGELOG.md and the entries in changelog.d/. See changelog.d/README.md.
#
#   tools/changelog/changelog.sh release <version> [<date>]
#       Moves every entry into CHANGELOG.md as the section "## [<version>] - <date>", updates the
#       comparison links at the bottom, and deletes the entries. The date defaults to today (UTC).
#   tools/changelog/changelog.sh preview
#       Prints the entries merged into one section's body, without changing anything.
#   tools/changelog/changelog.sh notes <version>
#       Prints the body of the section for <version>. If CHANGELOG.md has none, prints the entries
#       merged, as `preview` does (a pre-release is tagged before its entries are merged). Prints
#       nothing if there is neither.
#
# Run it from the repository root. It needs only bash, awk, sed and git.
set -euo pipefail

CHANGELOG=CHANGELOG.md
DIR=changelog.d
REPO_URL=https://github.com/rrgmc/ticktapdrum
# The Keep a Changelog headings, in the order a section lists them.
KINDS=(Added Changed Deprecated Removed Fixed Security)

die() {
  echo "changelog: $*" >&2
  exit 1
}

# The entry files, sorted by name. README.md explains the folder and is not an entry.
entries() {
  find "$DIR" -maxdepth 1 -type f -name '*.md' ! -name README.md | LC_ALL=C sort
}

# Prints the entries merged: one "### <kind>" heading per kind that has items, in KINDS order.
merged() {
  local files kind
  mapfile -t files < <(entries)
  [ "${#files[@]}" -gt 0 ] || return 0

  # Every "### " heading in an entry must be a known kind, and no item may come before one.
  awk -v kinds="${KINDS[*]}" '
    BEGIN { n = split(kinds, k, " "); for (i = 1; i <= n; i++) ok[k[i]] = 1 }
    FNR == 1 { kind = "" }
    /^### / {
      kind = substr($0, 5); sub(/[ \t]+$/, "", kind)
      if (!(kind in ok)) { printf "%s:%d: unknown heading \"%s\"\n", FILENAME, FNR, $0 > "/dev/stderr"; bad = 1 }
      next
    }
    /^[^ \t]/ && kind == "" {
      printf "%s:%d: text before the first ### heading\n", FILENAME, FNR > "/dev/stderr"; bad = 1
    }
    END { exit bad }
  ' "${files[@]}" || die "fix the entries above; the headings allowed are: ${KINDS[*]}"

  local first=1
  for kind in "${KINDS[@]}"; do
    # The lines under "### <kind>" in every entry. Blank lines are dropped, so the items of all
    # the entries form one list.
    local body
    body=$(awk -v want="$kind" '
      FNR == 1 { inside = 0 }
      /^### / { h = substr($0, 5); sub(/[ \t]+$/, "", h); inside = (h == want); next }
      inside && !/^[ \t]*$/ { print }
    ' "${files[@]}")
    [ -n "$body" ] || continue
    [ "$first" = 1 ] || echo
    first=0
    printf '### %s\n\n%s\n' "$kind" "$body"
  done
}

# Prints the body of "## [<version>]" in CHANGELOG.md, without blank lines at either end.
section() {
  awk -v v="$1" '
    /^## \[/ { inside = (index($0, "## [" v "]") == 1); next }
    /^\[[^]]*\]: / { inside = 0 }
    inside { line[++n] = $0; if ($0 !~ /^[ \t]*$/) { if (!first) first = n; last = n } }
    END { for (i = first; first && i <= last; i++) print line[i] }
  ' "$CHANGELOG"
}

release() {
  local version="${1:-}" date="${2:-$(date -u +%F)}"
  [ -n "$version" ] || die "usage: $0 release <version> [<date>]"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || die "\"$version\" is not a version such as 1.2.0"
  grep -q -F "## [$version]" "$CHANGELOG" && die "$CHANGELOG already has a section for $version"
  grep -q -x '## \[Unreleased\]' "$CHANGELOG" || die "$CHANGELOG has no \"## [Unreleased]\" heading"

  local body
  body=$(merged)
  [ -n "$body" ] || die "$DIR holds no entries; nothing to release"

  # The version this one follows: the newest "## [x.y.z]" heading already in the file.
  local previous
  previous=$(sed -n 's/^## \[\([0-9][^]]*\)\].*/\1/p' "$CHANGELOG" | head -1)

  local tmp
  tmp=$(mktemp)
  BODY="$body" awk -v v="$version" -v d="$date" -v prev="$previous" -v url="$REPO_URL" '
    /^## \[Unreleased\]$/ {
      print; print ""; print "## [" v "] - " d; print ""; print ENVIRON["BODY"]
      next
    }
    /^\[Unreleased\]: / {
      print "[Unreleased]: " url "/compare/v" v "...HEAD"
      if (prev != "") print "[" v "]: " url "/compare/v" prev "...v" v
      else print "[" v "]: " url "/releases/tag/v" v
      next
    }
    { print }
  ' "$CHANGELOG" > "$tmp"
  mv "$tmp" "$CHANGELOG"

  local files
  mapfile -t files < <(entries)
  if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    git rm -q --ignore-unmatch -- "${files[@]}"
  fi
  rm -f -- "${files[@]}"
  echo "Added $version to $CHANGELOG and removed ${#files[@]} entries from $DIR."
}

notes() {
  local version="${1:-}"
  [ -n "$version" ] || die "usage: $0 notes <version>"
  local body
  body=$(section "$version")
  if [ -z "$body" ]; then
    body=$(merged)
  fi
  [ -z "$body" ] || printf '%s\n' "$body"
}

[ -f "$CHANGELOG" ] || die "run this from the repository root, where $CHANGELOG is"

case "${1:-}" in
  release) shift; release "$@" ;;
  preview) merged ;;
  notes) shift; notes "$@" ;;
  *) die "usage: $0 release <version> [<date>] | preview | notes <version>" ;;
esac
