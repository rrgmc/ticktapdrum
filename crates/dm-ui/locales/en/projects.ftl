# Messages for projects. Ids start with "projects-".
# These report what saving, opening and exporting did. They show in the ☰ menu or the Export
# dialog. $name is a project's name as the person typed it; $path is a file's full path;
# $reason is why something failed, already in words.

# The name a project gets when it has none, for a saved file or an export, and the hint in the
# Save as name box.
projects-default-name = My beat

projects-new = Started a new project.
projects-saved = Saved "{ $name }".
projects-not-saved = Not saved: { $reason }
projects-opened = Opened "{ $name }".
projects-open-failed = "{ $name }" did not open: { $reason }
projects-deleted = Deleted "{ $name }".
projects-delete-failed = "{ $name }" was not deleted: { $reason }
projects-opened-file = Opened { $path }.
projects-open-file-failed = { $path } did not open: { $reason }
projects-saved-file = Saved to { $path }.
# $name is the style's name, such as Rock, which stays in English. "Play" is the strip's Play
# button.
projects-preset-started = Started from the { $name } beat. Press Play to hear it.

## Exports. $place is either a full path or what projects-saved-where gives.

projects-midi-saved = Saved the MIDI file to { $place }.
projects-midi-failed = The MIDI file was not saved: { $reason }
projects-wav-saved = Saved the recording to { $place }.
projects-wav-failed = The recording was not saved: { $reason }
projects-style-saved = Saved the keyboard style to { $place }.
# $reason is a keyboard-problem message or the system's reason the file could not be written.
projects-style-failed = The keyboard style was not saved: { $reason }
# An exported file's name, then where a phone keeps exports, such as "the Files app, under On My
# iPhone, …".
projects-saved-where = { $file } in { $place }
# Reasons a recording was not saved.
projects-export-cancelled = you stopped it.
projects-export-nothing = there is nothing to play.
projects-export-stopped = the recording stopped unexpectedly.

## The kinds of file a desktop's save and open dialogs list.

projects-file-kind-project = TickTapDrum project
projects-file-kind-midi = MIDI file
projects-file-kind-wav = WAV recording
projects-file-kind-style = Keyboard style
