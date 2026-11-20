# Messages for menu. Ids start with "menu-".
# The menu opens under the ☰ button. Its items are about 240 points wide.

## Strip buttons the menu offers when the strip has no room for them.

menu-undo = Undo
menu-redo = Redo
menu-tempo-tip = Tempo and swing

## The status at the top of the menu.

# Instead of the project's name, before it has been saved under one.
menu-no-name = No name yet
menu-unsaved = Changes not saved yet. They stay on this device anyway.

## The menu's items. Each is also the title of the dialog it opens.

menu-save = Save
menu-save-as = Save as
menu-open = Open
# Starts a new, empty project.
menu-new = New
# Opens a list of ready-made beats in well-known styles (rock, funk and so on).
menu-presets = Load default style
# A drum tab is drums written as text,
# one row per drum, such as "SD |----o-------o---|".
menu-paste-tab = Paste a drum tab
menu-sounds = Sounds
menu-song = Song
menu-export = Export
# Locks the screen in landscape orientation.
menu-landscape = Keep in landscape
# A setting: while MIDI in is on, each row's name shows the keyboard key that plays it, such as C1.
menu-key-names = Keyboard notes
menu-key-names-tip = With MIDI in on, show beside each drum the keyboard note that plays it.
# Shows the bars as small pictures under the buttons all the time.
menu-bar-line = Show bars under the buttons
menu-language = Language
# The first choice in the Language dialog: follow the system's language. $language is that
# language's own name, such as "English".
menu-language-system = System ({ $language })
# Sends the drums to another device or program over MIDI. $device is the chosen output's name,
# as the system reports it and shortened to fit, or menu-midi-output-none.
menu-midi-output-choice = MIDI output: { $device }
# The title of the dialog that chooses the MIDI output.
menu-midi-output = MIDI output
# The first choice in the MIDI output dialog: no output, so the drums play on this device.
menu-midi-output-none = None
menu-midi-output-about = The drums play on the chosen output instead of on this device.
# A chosen output that is not there now. $device is its name, as the system reported it.
menu-midi-output-not-connected = { $device } (not connected)
# Under the list, when the chosen output is listed but not connected (yet).
menu-midi-output-waiting = { $device } is not connected yet.
# The volume slider under the list, shown while an output is chosen.
menu-midi-volume = Volume
menu-midi-volume-about = The chosen output plays every drum at this volume. Each drum's own volume still applies.
# One-line messages in the menu's status. $device is the output's name.
menu-midi-sending = Sending to { $device }.
menu-midi-gone = { $device } is gone. Playing on this device again.
menu-midi-in-use = { $device } is in use by another program.
# A one-line message when the MIDI input goes away while recording. $device is its name.
menu-midi-input-gone = { $device } is gone. Recording stopped.
# Where the drums played on a MIDI device come from. $device is the chosen input's name, as the
# system reports it and shortened to fit, or menu-midi-input-same.
menu-midi-input-choice = MIDI input: { $device }
# The title of the dialog that chooses the MIDI input.
menu-midi-input = MIDI input
# The first choice in the MIDI input dialog: the input of the MIDI output's own device.
menu-midi-input-same = Same as output
menu-midi-input-about = Where the drums you play come from. Same as output fits a keyboard; choose another for drum pads.
# A chosen input that is not there now. $device is its name, as the system reported it.
menu-midi-input-not-connected = { $device } (not connected)
# The title of the dialog that sets the tempo and the swing.
menu-tempo = Tempo

## Questions asked before something unsaved is lost.

menu-new-question = Start a new beat? This beat has changes that are not saved, and they go.
menu-start-new = New
# Answers "no" to a question that would lose this beat's unsaved changes.
menu-keep-this = Cancel
menu-open-file-question = Open a file? This beat's unsaved changes go.
menu-choose-file = Choose file
menu-open-question = Open "{ $name }"? This beat's unsaved changes go.
# $name is the name of a saved project, as the person typed it.
menu-delete-question = Delete "{ $name }" for good?
menu-delete = Delete
# Answers "no" to "Delete for good?".
menu-keep = Cancel
menu-cancel = Cancel

## The Open dialog.

# Desktop only: opens a project file from anywhere on the computer.
menu-open-file = Open a file
# Desktop only: saves the project as a file anywhere on the computer.
menu-save-file = Save to a file
menu-nothing-saved = Nothing saved yet.

## The Save as dialog.

# The label above the box where the project's name is typed.
menu-name = Name
# $name is the name being typed, which a saved project already has.
menu-replaces = This replaces the saved "{ $name }".

## The Load default style dialog. Style names such as Rock stay in English.

menu-presets-intro = A ready-made beat to play with and change. It replaces the beat on screen.
menu-preset-question = Load { $name }? This beat's unsaved changes go.
menu-start = Load
menu-preset-rock = A steady straight beat with the kick on 1 and 3, the snare on 2 and 4, and a drum fill every fourth bar.
menu-preset-pop = A bright, even beat with snare and clap together on 2 and 4 over busy hi-hats.
menu-preset-hip-hop = A laid-back boom bap groove with a heavy kick, a hard snare and a little swing.
menu-preset-funk = A busy groove with syncopated kicks, accented hi-hats and quiet ghost notes on the snare.
menu-preset-bossa-nova = A gentle Brazilian rhythm with a rocking kick and a soft snare clave over two bars.
menu-preset-house = A dance beat with the kick on every beat, claps on 2 and 4 and open hi-hats in between.
menu-preset-reggae = A relaxed one drop with kick and snare together on beat 3 and shuffled hi-hats.

## The Export dialog.

menu-recording = Recording the song to a WAV file
# Stops the recording in progress.
menu-stop = Stop
menu-midi = MIDI file
menu-midi-about = The notes, for other music software.
menu-wav = WAV recording
menu-wav-about = The sound, for any music player.
# Shown while the drum sounds are still downloading, so the WAV button is greyed out.
menu-wav-about-no-bank = The sound, for any music player. It needs the drum sounds, which are not loaded yet.
# $place is where a phone puts exported files, such as "the Files app, under On My iPhone, …".
menu-export-place = Exports are saved in { $place }.
# A small heading over the two keyboard style buttons. An arranger keyboard plays a backing
# "style" that follows the player's chords.
menu-for-keyboard = For a keyboard
# A button. Always "Keyboard style", never "Style" alone: the app already calls its ready-made
# beats styles. It also titles the step that chooses the sections.
menu-keyboard-style = Keyboard style
# The line under Keyboard style, for Yamaha keyboards. "User" is the keyboard's own button name,
# as printed on it. Only these lines name a brand; the buttons never do.
menu-keyboard-style-about-yamaha = A style for a Yamaha arranger keyboard, such as the PSR-SX720. Copy it to a USB stick and choose it under User.
# A button that sends the style straight to the keyboard. It also titles the step that chooses
# the sections.
menu-keyboard-send = Send to keyboard
menu-keyboard-send-about = Puts the style straight into the keyboard's User styles, through the MIDI output.
# Under Send to keyboard while it is greyed out because no MIDI output is connected. "Settings"
# and "MIDI output" are the menu's group and item.
menu-keyboard-send-about-no-output = Choose the keyboard under Settings, MIDI output first.

## The Settings group at the bottom of the menu.

menu-settings = Settings
# A heading in the menu that opens to show the items for adding grooves and drum tabs.
menu-import = Import
# How each row is labelled in the grid: by its name, a picture of the drum, or both.
menu-rows-names = Instruments: names
menu-rows-pictures-and-names = Instruments: pictures and names
menu-rows-pictures = Instruments: pictures

## The Paste a drum tab dialog. Its title is menu-paste-tab.

menu-paste-tab-hint = Paste a few bars of a drum tab, with the drum names at the start of each row.
# Adds the pasted tab to the project as a new pattern.
menu-paste-tab-add = Add as pattern
# A checkbox, off by default. When on, bars that cannot be read, such as triplet fills, are left
# out instead of refusing the whole tab.
menu-paste-tab-skip-odd-bars = Leave out bars that do not fit
# A heading over links to web sites with drum tabs, in the Paste a drum tab dialog.
menu-paste-tab-sources = Where to find drum tabs
menu-paste-tab-source-ultimate-guitar = Ultimate Guitar: drum tabs
menu-paste-tab-source-tabcrawler = Tabcrawler

# Shown with a progress bar while a style goes to the keyboard over MIDI.
menu-keyboard-sending = Sending the style to the keyboard
