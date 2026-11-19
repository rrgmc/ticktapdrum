# Getting the sound bank: the offer to download it, the download, loading and errors.
# Ids start with "bank-". $bank is the bank's name, which is not translated.

bank-offer-title = Drum sounds needed
bank-offer = TickTapDrum plays its sounds from an instrument bank, { $bank }. It is downloaded once and kept on this device.
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
