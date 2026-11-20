# Changelog entries

Each pull request that changes what people who use the app see adds one file here. The release
merges them all into [`CHANGELOG.md`](../CHANGELOG.md) and deletes them. Separate files keep two
pull requests from editing the same lines, so they do not conflict.

## Writing an entry

Name the file after the change, such as `pattern-rename.md`. Any name ending in `.md` works,
except `README.md`.

Use the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) headings, at level three, with a
list under each:

```markdown
### Added

- Rename in the pattern pop-up, to choose a pattern's letter.

### Fixed

- Each MIDI out hit ends with its own note-off.
```

The headings allowed are `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed` and `Security`.
Leave out the ones you do not need. Write for people who use the app, not for developers. A change
they cannot see, such as a refactor, a test or a CI change, needs no entry.

To see how the entries will read once merged:

```sh
tools/changelog/changelog.sh preview
```

## At a release

The version pull request runs:

```sh
tools/changelog/changelog.sh release 1.2.0
```

This adds a `## [1.2.0] - <today>` section to `CHANGELOG.md`, updates its comparison links, and
deletes the entries. The release workflow uses that section as the GitHub release notes.

A pre-release, such as `1.2.0-rc.1`, is usually tagged without merging. Its release notes are then
the entries as they stand on the tagged commit.
