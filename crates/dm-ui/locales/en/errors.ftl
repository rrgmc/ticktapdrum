# Messages for errors. Ids start with "errors-".

## Files the system could not read or write. $detail is the system's own description of the
## problem, such as "Permission denied (os error 13)". These follow a colon in a longer message,
## such as "Not saved: could not write the file: ...", so they start in lower case.

errors-read-file = could not read the file: { $detail }
errors-write-file = could not write the file: { $detail }
errors-delete-file = could not delete the file: { $detail }
errors-create-folder = could not create the folder: { $detail }

## Project files that did not load. These follow a colon in a longer message, such as
## "My beat did not open: ...".

# $detail is the JSON reader's description of the problem, in English.
errors-not-project = This file is not a TickTapDrum project. It could not be read ({ $detail }).
# $format is a whole number.
errors-newer-format = This file was made by a newer version of TickTapDrum (file format { $format }). Update the app to open it.
# $detail is the JSON reader's description of what is missing or wrong, in English.
errors-damaged = This file is damaged or incomplete: { $detail }.
errors-not-an-object = This file is damaged or incomplete: it does not hold a project.
errors-format-missing = This file is damaged or incomplete: the file format number is missing.
errors-format-zero = This file is damaged or incomplete: the file format number is 0.
errors-format-not-whole = This file is damaged or incomplete: the file format number is not a whole number.
errors-no-rows = This file is damaged or incomplete: the project has no drum rows.
# $count is always more than $max.
errors-too-many-rows = This file is damaged or incomplete: the project has { $count } drum rows, and the most allowed is { $max }.
errors-no-patterns = This file is damaged or incomplete: the project has no patterns.
# $count is always more than $max.
errors-too-many-patterns = This file is damaged or incomplete: the project has { $count } patterns, and the most allowed is { $max }.
# $count is always more than $max. An entry is one pattern played in the song.
errors-song-too-long = This file is damaged or incomplete: the song has { $count } entries, and the most allowed is { $max }.
# $id is a number inside the file that names a row.
errors-duplicate-row-id = This file is damaged or incomplete: two drum rows have the id { $id }.
# $name is the name the file gives a pattern.
errors-bad-pattern-name = This file is damaged or incomplete: "{ $name }" is not a pattern name, which must be one letter from A to Z.
# $name is a pattern letter, such as A.
errors-duplicate-pattern = This file is damaged or incomplete: two patterns are named { $name }.
# $name is the name of a pattern.
errors-song-pattern-missing = This file is damaged or incomplete: the song plays pattern "{ $name }", which is not in the file.
