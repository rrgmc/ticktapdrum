# The groove list and groove import. Ids start with "grooves-".
# A groove is a short drum loop from a MIDI file. A pack is a zip of many such files, or one
# file. $name is a file's name as people know it, such as "Chilled Beat.zip", or a groove's or
# pack's name.

## The ☰ menu's items, and the groove list dialog.

# Opens a MIDI file or a zip of them, and keeps its grooves in the groove list.
grooves-add-from-file = Add grooves from a file
# The groove list's menu item and dialog title.
grooves-title = Grooves
grooves-intro = Tap Play to hear a groove. Add puts it in your project as a new pattern.
grooves-empty = No grooves yet.
# A pack's heading: its name and how many grooves it holds.
grooves-pack = { $name } ({ $count })
# How long a groove is, in bars.
grooves-bars = { $count ->
    [one] 1 bar
   *[other] { $count } bars
}
grooves-play = Play
grooves-stop = Stop
# Adds the groove to the project as a new pattern.
grooves-add = Add
grooves-add-tip = Add as a new pattern
grooves-delete = Delete
# Answers a delete question: do not delete.
grooves-keep = Cancel
grooves-delete-pack-question = Delete all the grooves of { $name }?
grooves-delete-groove-question = Delete the groove { $name }?
# $reason is why, already in words.
grooves-delete-failed = Not deleted: { $reason }
# The kind of file a desktop's open dialog lists.
grooves-file-kind = MIDI files and zips

## What an import did.

grooves-added = { $count ->
    [one] Added 1 groove from { $name }.
   *[other] Added { $count } grooves from { $name }.
}
# Added after grooves-added. $reasons is a list such as "not in 4/4 (3); no notes (1)".
grooves-left-out = { $refused ->
    [one] 1 file was left out: { $reasons }.
   *[other] { $refused } files were left out: { $reasons }.
}
# Added when a pack holds the same grooves for several drum programs.
grooves-only-general-midi = Only the General MIDI version was used.
# One file that was refused. $reason is one of the grooves-refusal- messages.
grooves-not-added = { $name } was not added: { $reason }.
# Every file of a pack was refused.
grooves-none-added = Nothing was added from { $name }. { $refused } files were left out: { $reasons }.
grooves-none-found = Nothing was added from { $name }: it holds no grooves.
# One reason and how many files it left out, in a list.
grooves-reason-count = { $reason } ({ $count })
# $reason is the system's own description of what went wrong.
grooves-read-failed = { $name } could not be read: { $reason }
grooves-not-saved = The grooves were not saved: { $reason }

## Why a file is not a groove the app takes, in words for beginners.

grooves-refusal-not-midi = not a MIDI file
grooves-refusal-no-notes = no notes
grooves-refusal-several-channels = a whole song, not a groove
grooves-refusal-not-four-four = not in 4/4
grooves-refusal-tempo-changes = the tempo changes
grooves-refusal-too-long = longer than 16 bars
grooves-refusal-off-grid = played freely, off the grid
grooves-refusal-other-drum-program = made for another drum program (try the pack's General MIDI version)

## Why a file could not be imported at all.

grooves-error-not-grooves = { $name } is not a MIDI file or a zip.
grooves-error-too-large = { $name } is too large.
grooves-error-broken-zip = { $name } is a damaged zip file.
grooves-error-no-midi-files = { $name } holds no MIDI files.

## What adding a groove to the project did.

# $pattern is the new pattern's letter, such as C. $name is the groove's name.
grooves-added-pattern = { $bars ->
    [one] Added pattern { $pattern }: { $name }, 1 bar.
   *[other] Added pattern { $pattern }: { $name }, { $bars } bars.
}
# The groove's bars were one shorter groove played again and again.
grooves-repeats = Its { $given } bars repeat every { $bars }, so the pattern has those.
grooves-bars-left-out = { $count } bars after the first 16 were left out.
grooves-rows-added = { $count ->
    [one] 1 row was added for a new drum.
   *[other] { $count } rows were added for new drums.
}
grooves-drums-left-out = { $count } drums were left out: the project has no room for more rows.
grooves-no-room = No room for another pattern: a project holds 26.
grooves-add-failed = The groove was not added.
