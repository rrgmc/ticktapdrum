# Getting the sound bank: the offer to download it, the download, loading and errors.
# Ids start with "bank-". $bank is the bank's name, which is not translated.

bank-offer-title = Get the drum sounds
bank-offer = To play your beats, TickTapDrum needs a set of recorded drum sounds called { $bank }. Download it once and it stays on this device.
# Sizes in whole mebibytes (MiB): the download, and the space the bank takes once installed.
bank-offer-sizes = Download: { $download } MiB. Space used: { $space } MiB.
# The licence of the bank ColomboGMGS2.
bank-license-colombo = Licence: CC-BY-SA 4.0, claimed in the file; its credits name other banks among its sample sources.
# The licence of another bank. $license is the bank's own English text.
bank-license = Licence: { $license }.
bank-download = Download

bank-installing-title = Getting the drum sounds
bank-connecting = Connecting
# Progress, in whole mebibytes (MiB).
bank-downloading = Downloading: { $done } MiB of { $total } MiB
bank-checking-download = Checking the download
# Taking the sound file out of the downloaded zip archive.
bank-unpacking = Unpacking: { $done } MiB of { $total } MiB
bank-checking-sounds = Checking the sounds
bank-cancel = Cancel

bank-loading-title = Loading the drum sounds

bank-failed-title = Something went wrong
bank-try-again = Try again
# In the errors, $detail is the system's own description of the problem, often in English.
bank-error-download = The download failed: { $detail }
bank-error-mismatch-archive = The downloaded file was not the expected one, so it was deleted.
bank-error-mismatch-bank-file = The unpacked sound file was not the expected one, so it was deleted.
bank-error-archive = The downloaded file could not be unpacked: { $detail }
bank-error-file = A file on this device could not be read or written: { $detail }
bank-error-cancelled = The download was cancelled.
bank-error-soundfont = The drum sounds could not be loaded: { $detail }
bank-error-device = The audio output could not be opened: { $detail }
bank-error-nothing = There is nothing to play.
bank-error-render-cancelled = The render was cancelled.
bank-error-installer-stopped = The download stopped unexpectedly.
bank-error-loading-stopped = Loading the drum sounds stopped unexpectedly.

# A SoundFont of one's own. $name is the file's name without .sf2, which is not translated.
bank-use-own = Use your own SoundFont file
bank-use-added = Use { $name }
bank-adding-title = Adding a SoundFont
bank-add-cancelled = Adding the SoundFont was cancelled.
# $reason is one of the errors above, or the system's own words.
bank-add-failed = { $name } could not be added. { $reason }
bank-add-no-kits = { $name } has no drum kits, so it was not added.
bank-add-busy = A SoundFont is already being added. Wait for it to finish.
bank-chosen-missing = The SoundFont { $name } is no longer on this device, so the app went back to its own drum sounds.
bank-load-failed = { $name } did not load, so the app went back to its own drum sounds. { $reason }
bank-delete-failed = { $name } could not be deleted. { $reason }

# How the project's kit plays in the SoundFont playing. These sentences are joined into one
# notice. $bank is a SoundFont's name. $kit is the kit the project asks for; $played is the kit
# that plays instead.
bank-now-playing = Now playing { $bank }.
bank-made-with = Made with { $bank }.
bank-kit-plays-as = Its kit { $kit } plays as { $played } here.
bank-sounds-may-differ = Its sounds may differ here.
# A kit whose name is not known, by its General MIDI program number.
bank-kit-program = kit { $program }
