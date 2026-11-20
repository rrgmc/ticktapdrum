//! What the Paste a drum tab dialog reports, in words. The dialog is in `menu.rs`; reading the
//! tab and adding it to the project is `dm-tab`'s work.

use dm_tab::{AddError, Added, Refusal, Tab};

/// Why a pasted tab was not added, ready to show in the dialog.
pub(crate) fn refusal(refusal: &Refusal) -> String {
    match refusal {
        Refusal::NoTab => tr!("patterns-tab-no-tab"),
        Refusal::OddBars(widths) => tr!("patterns-tab-odd-bars", widths = join(widths)),
        Refusal::UnknownDrums(labels) => {
            tr!("patterns-tab-unknown-drums", labels = labels.join(", "))
        }
        Refusal::Misaligned => tr!("patterns-tab-misaligned"),
    }
}

/// Why a tab that was read could not be added.
pub(crate) fn add_error(error: &AddError) -> String {
    match error {
        AddError::NoFreePattern => tr!("patterns-tab-no-room"),
    }
}

/// What was added, then a sentence for each part of the tab that was left out.
pub(crate) fn added(added: &Added, tab: &Tab, pattern: char) -> String {
    let mut message = tr!(
        "patterns-tab-added",
        pattern = pattern.to_string(),
        bars = added.bars
    );
    let mut notes = Vec::new();
    if added.bars_left_out > 0 {
        notes.push(tr!(
            "patterns-tab-bars-left-out",
            count = added.bars_left_out
        ));
    }
    if tab.odd_bars_left_out > 0 {
        notes.push(tr!(
            "patterns-tab-odd-bars-left-out",
            count = tab.odd_bars_left_out
        ));
    }
    if !tab.dropped_labels.is_empty() {
        notes.push(tr!(
            "patterns-tab-labels-left-out",
            labels = tab.dropped_labels.join(", ")
        ));
    }
    if added.drums_left_out > 0 {
        notes.push(tr!(
            "patterns-tab-drums-left-out",
            count = added.drums_left_out
        ));
    }
    for note in notes {
        message.push(' ');
        message.push_str(&note);
    }
    message
}

fn join(numbers: &[usize]) -> String {
    numbers
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
