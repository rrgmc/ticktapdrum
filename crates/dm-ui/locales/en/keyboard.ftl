## The keyboard style step, which chooses the pattern for each section of the style. Section
## names follow what arranger keyboards print on their buttons.

# Above the list of sections.
keyboard-sections-about = Choose a pattern for each part of the style. Main A is needed.
# The section that plays once before the beat starts.
keyboard-intro = Intro
# One of the four main beats. $letter is A, B, C or D, as on the keyboard's buttons.
keyboard-main = Main { $letter }
# The one-bar fill played when moving to Main $letter.
keyboard-fill = Fill { $letter }
# A one-bar pause or hit that breaks the beat.
keyboard-break = Break
# The section that plays once to finish.
keyboard-ending = Ending
# The choice that leaves a section out.
keyboard-none = None
# Under a fill left at None. $letter is the main's letter.
keyboard-fill-from-main = The last bar of Main { $letter } plays as its fill.
# Why the finishing button is greyed out. One of these shows at a time.
keyboard-problem-no-main-a = Choose a pattern for Main A.
keyboard-problem-not-one-bar = Fills and the Break must be patterns one bar long.
keyboard-problem-no-such-pattern = A chosen pattern is gone. Choose it again.
# The finishing buttons: save the style as a file, or send it to the keyboard.
keyboard-save = Save
keyboard-send = Send
