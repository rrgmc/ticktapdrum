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

## The Add grooves from a link dialog. A link is a web address, such as https://example.com/a.mid.

# The menu item and the dialog's title. Downloads a MIDI file or a zip from a link.
grooves-add-from-link = Add grooves from a link
grooves-link-hint = Paste a link to a groove file (.mid) or pack (.zip).
# Starts the download.
grooves-link-add = Add
# While downloading. $kb is a whole number of kilobytes; $mb has one decimal.
grooves-link-downloading-kb = Downloading... { $kb } KB
grooves-link-downloading-mb = Downloading... { $mb } MB
# Why a download failed.
grooves-link-not-a-link = This is not a link.
grooves-link-failed = The download failed. Check the link and the connection.
# $code is the web server's error number, such as 404.
grooves-link-status = The server answered with an error ({ $code }).
grooves-link-too-large = The file is too large.
grooves-link-web-page = This is a web page, not a groove file. Copy the link of the download itself, often by long-pressing the download button.
# Text other than a link was shared to the app, such as from another app's Share menu.
grooves-link-share-refused = Only links to groove files can be shared to TickTapDrum.

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
grooves-repeats = The file repeats the same { $bars ->
    [one] bar
   *[other] { $bars } bars
} over its { $given } bars, so the pattern keeps just { $bars ->
    [one] that bar
   *[other] those
}.
grooves-bars-left-out = { $count } bars after the first 16 were left out.
grooves-rows-added = { $count ->
    [one] 1 row was added for a new drum.
   *[other] { $count } rows were added for new drums.
}
grooves-drums-left-out = { $count } drums were left out: the project has no room for more rows.
grooves-no-room = No room for another pattern: a project holds 26.
grooves-add-failed = The groove was not added.
# A heading over links to free grooves, in the Add grooves from a link dialog.
grooves-sources = Free grooves
# Above links that open web pages in the browser.
grooves-sources-hint = These pages open in the browser. Copy a download link there and paste it above.
grooves-source-magenta-page = Groove MIDI Dataset (Google Magenta)
grooves-source-groove-monkee = Groove Monkee: free MIDI loops
grooves-source-beat-farm = Groove Monkee: Beat Farm
# The dialog listing the grooves that come with the app, and the button that opens it in the
# Ready-made styles dialog. Google Magenta and Groove MIDI Dataset are names; keep them.
grooves-magenta-title = Grooves by real drummers
grooves-magenta-intro = Beats played by drummers on an electronic kit, a few for each style. Tap Play to hear one; Load starts a new beat from it, as a ready-made style does.
# The credits, required by the licence. Keep the names, the year and "CC BY 4.0".
grooves-magenta-credits = From the Groove MIDI Dataset by Google Magenta (Gillick, Roberts, Engel, Eck and Bamman, 2019), licensed under CC BY 4.0. Chosen and changed for TickTapDrum: the first bars of each take, with the notes moved onto the 16-step grid.
grooves-magenta-page = The Groove MIDI Dataset
grooves-magenta-license = The CC BY 4.0 licence
# Hover text of the Load button beside a groove that comes with the app.
grooves-magenta-load-tip = Start a new beat from this groove
