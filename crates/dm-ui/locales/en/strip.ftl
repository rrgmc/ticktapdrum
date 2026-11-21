# Messages for strip. Ids start with "strip-".
# The strip is the one row of buttons at the top of the screen. Its words must be short: when
# they do not fit, the buttons show icons only and the words become hover text.

strip-play = Play
# Play while in song mode: plays the whole song rather than one pattern.
strip-play-song = Play song
strip-stop = Stop
# Hover text on Play while the drums go to a MIDI output instead of this device's speakers.
# $device is the output's name, as the system reports it.
strip-midi-tip = The drums play on { $device }.
# Hover text on Play while a MIDI output is chosen but not connected, so the drums play on this
# device. $device is the output's name, as the system reports it.
strip-midi-missing-tip = { $device } is not connected. The drums play here until it is.
# Hover text on Play while the MIDI input is on and connected: playing a pattern records the
# hits from it. $device is the input's name, as the system reports it.
strip-record-tip = Playing records from { $device }.
# The MIDI in button, shown while the MIDI input is connected. On, playing a pattern records what
# is played on the input. $device is the input's name, as the system reports it.
strip-midi-in = MIDI in
strip-midi-in-off-tip = Record from { $device } while a pattern plays.
strip-midi-in-on-tip = Recording from { $device } while a pattern plays. Tap to stop.
strip-patterns = Patterns
strip-tempo-tip = Tempo and swing
# Opens the dialog that chooses the drum kit. Same as menu-sound-kits.
strip-sound-kits = Sound kits
strip-undo = Undo
strip-redo = Redo
strip-menu = Menu

# The bar button, when the grid shows one bar of a pattern.
strip-bar = Bar { $first } of { $total }
# The bar button, when the grid shows several bars side by side.
strip-bars = Bars { $first }-{ $last } of { $total }
# The arrows either side of the bar button: show the steps before or after.
strip-back = Back
strip-forward = Forward
# The same arrows at the first or last bar, where they wrap round to the other end.
strip-to-end = To the end
strip-to-start = Back to the start

# The step levels button. Its word names which loudness levels a tap on a step goes through:
# only on and off; on, soft and off; or on, loud, soft and off.
strip-levels-on-off = On/off
strip-levels-soft = Soft
strip-levels-loud = Loud
# The step levels button's hover text, one for each choice.
strip-levels-tip-on-off = { strip-levels-on-off }: A tap turns a step on or off. Tap here for soft steps too.
strip-levels-tip-soft = { strip-levels-soft }: A tap goes on, soft, off. Tap here for loud steps too.
strip-levels-tip-loud = { strip-levels-loud }: A tap goes on, loud, soft, off. Tap here for just on and off.
