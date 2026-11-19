//! Drum names in the language of the interface, and row names shown through them.
//!
//! The project stores each row's name as text. A new row is named after its drum in English, as
//! `dm_project::drum_name` gives it. Such a name is shown in the current language, so a project
//! made in one language reads naturally in another. A name the person typed is shown as typed.

use dm_project::GM_DRUMS;
use fluent_bundle::FluentArgs;

use crate::i18n::{self, Language};

/// The name of the General MIDI percussion key `key` in the current language.
pub(crate) fn drum_name(key: u8) -> String {
    drum_name_in(i18n::language(), key)
}

/// The name of `key` in `language`. Keys outside General MIDI get a generic name with the number.
fn drum_name_in(language: Language, key: u8) -> String {
    if GM_DRUMS.iter().any(|(k, _)| *k == key) {
        i18n::tr_in(language, &format!("drums-name-{key}"), None)
    } else {
        let mut args = FluentArgs::new();
        args.set("key", key);
        i18n::tr_in(language, "drums-name-other", Some(&args))
    }
}

/// How to show a row named `name` that plays `key`: in the current language when the name is
/// the drum's own name in any language, and unchanged when the person renamed the row.
pub(crate) fn row_name(name: &str, key: u8) -> String {
    row_name_in(i18n::language(), name, key)
}

fn row_name_in(language: Language, name: &str, key: u8) -> String {
    let default = name == dm_project::drum_name(key)
        || Language::ALL
            .into_iter()
            .any(|other| name == drum_name_in(other, key));
    if default {
        drum_name_in(language, key)
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drum_has_a_name_in_every_language() {
        for language in Language::ALL {
            for (key, _) in GM_DRUMS {
                let name = drum_name_in(language, key);
                assert!(!name.starts_with("drums-"), "{} {key}", language.tag());
            }
        }
    }

    #[test]
    fn the_english_names_are_the_projects_names() {
        for (key, name) in GM_DRUMS {
            assert_eq!(drum_name_in(Language::English, key), name);
        }
    }

    #[test]
    fn an_untouched_english_name_shows_in_the_language() {
        assert_eq!(
            row_name_in(Language::BrazilianPortuguese, "Kick", 36),
            "Bumbo"
        );
        assert_eq!(row_name_in(Language::English, "Kick", 36), "Kick");
    }

    #[test]
    fn a_portuguese_default_name_shows_in_the_language() {
        assert_eq!(row_name_in(Language::English, "Caixa", 38), "Snare");
        assert_eq!(
            row_name_in(Language::BrazilianPortuguese, "Caixa", 38),
            "Caixa"
        );
    }

    #[test]
    fn a_renamed_row_keeps_its_name() {
        for language in Language::ALL {
            assert_eq!(row_name_in(language, "My kick", 36), "My kick");
            // Another drum's name is a name the person chose for this row.
            assert_eq!(row_name_in(language, "Snare", 36), "Snare");
        }
        assert_eq!(row_name("Groove", 42), "Groove");
    }

    #[test]
    fn an_unknown_key_gets_a_generic_name() {
        assert_eq!(row_name_in(Language::English, "Drum 20", 20), "Drum 20");
        assert_eq!(
            row_name_in(Language::BrazilianPortuguese, "Drum 20", 20),
            "Tambor 20"
        );
        assert_eq!(row_name_in(Language::English, "Tambor 20", 20), "Drum 20");
    }
}
