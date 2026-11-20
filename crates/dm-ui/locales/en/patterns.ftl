# The pattern pop-up. Ids start with "patterns-".

# $name is the pattern's letter, such as A.
patterns-delete-question = Delete pattern { $name }? Its steps go too.
patterns-delete = Delete
# Answers the question: do not delete the pattern.
patterns-keep = Cancel
# Adds a new, empty pattern.
patterns-new = New
# Adds a copy of the pattern being edited.
patterns-copy = Copy

## What adding a pasted drum tab did, shown in the menu, and why a tab was not added, shown in the
## Paste a drum tab dialog. A tab's bars are its measures.

# $pattern is the new pattern's letter, such as C. $bars is how many bars it has.
patterns-tab-added = Added pattern { $pattern } from the tab: { $bars } bars.
# Added after patterns-tab-added when part of the tab was left out. A pattern holds 16 bars.
patterns-tab-bars-left-out = { $count } bars after the first 16 were left out.
# $labels are the names at the start of the rows, as written in the tab, separated by commas.
patterns-tab-labels-left-out = Rows without a known drum were left out: { $labels }.
patterns-tab-drums-left-out = { $count } drums were left out: the project has no room for more rows.
# "x---o---" is an example of a row's hits (x, o) and rests (-). Keep it as it is.
patterns-tab-no-tab = No drum tab found. Each row needs a drum name, then hits and rests such as x---o---.
# $widths are the widths of the odd bars, in characters, separated by commas.
patterns-tab-odd-bars = Some bars are { $widths } characters wide. Only bars of 4, 8, 16 or 32 characters can be read; triplets cannot.
patterns-tab-unknown-drums = Unknown drum names: { $labels }.
patterns-tab-misaligned = The rows of the tab do not line up: some have more bars than others.
patterns-tab-no-room = No room for another pattern: a project holds 26.
