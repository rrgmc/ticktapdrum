//! Translations. Every piece of text the interface shows comes from a Fluent message file under
//! `locales/<language>/`, looked up with [`tr!`](crate::tr).
//!
//! The language follows the system unless the person picks one in the ☰ menu. The choice is a
//! device setting, kept in the store.
//!
//! To add a language: add a variant to [`Language`], copy `locales/en` to a folder named after
//! its tag, translate every file, and list the folder in [`resources`]. The tests check that every
//! language has the same messages with the same variables.
//!
//! Message ids are kebab-case and start with the file's area, such as `menu-undo` in `menu.ftl`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// A language the interface is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    BrazilianPortuguese,
}

impl Language {
    /// Every language, in the order the menu offers them.
    pub const ALL: [Language; 2] = [Language::English, Language::BrazilianPortuguese];

    /// The BCP 47 tag, which is also the name of the folder under `locales`.
    pub fn tag(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::BrazilianPortuguese => "pt-BR",
        }
    }

    /// The language's name in that language, as the menu shows it whatever the current one is.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::BrazilianPortuguese => "Português (Brasil)",
        }
    }

    /// The language with exactly this tag, as [`Language::tag`] gives it.
    pub fn from_tag(tag: &str) -> Option<Language> {
        Language::ALL.into_iter().find(|l| l.tag() == tag)
    }

    /// The language that best serves someone whose system asks for `tag`, such as `pt-PT`,
    /// `en_US` or `pt-BR-u-mu-celsius`. Only the language part decides: every Portuguese
    /// gets Brazilian Portuguese, since it is the only Portuguese there is.
    pub fn matching(tag: &str) -> Option<Language> {
        let primary = tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Language::English),
            "pt" => Some(Language::BrazilianPortuguese),
            _ => None,
        }
    }

    fn index(self) -> usize {
        match self {
            Language::English => 0,
            Language::BrazilianPortuguese => 1,
        }
    }
}

/// The first of the system's preferred languages that the interface is translated into.
/// Android has no reliable way to ask from here; its host passes the activity's language to
/// [`DrumApp::with_system_locale`](crate::DrumApp::with_system_locale) instead.
pub(crate) fn system_language() -> Option<Language> {
    #[cfg(not(target_os = "android"))]
    {
        sys_locale::get_locales().find_map(|tag| Language::matching(&tag))
    }
    #[cfg(target_os = "android")]
    {
        None
    }
}

/// The language text is shown in, as an index into [`Language::ALL`].
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Shows text in `language` from now on.
pub(crate) fn set_language(language: Language) {
    CURRENT.store(language.index() as u8, Ordering::Relaxed);
}

/// The language text is shown in.
pub fn language() -> Language {
    Language::ALL[CURRENT.load(Ordering::Relaxed) as usize]
}

/// Defines [`resources`] from one list of the files every language has.
macro_rules! locale_files {
    ($($file:literal),+ $(,)?) => {
        /// The message files of a language, in a fixed order, as (file name, contents).
        pub(crate) fn resources(language: Language) -> Vec<(&'static str, &'static str)> {
            match language {
                Language::English => vec![
                    $(($file, include_str!(concat!("../locales/en/", $file)))),+
                ],
                Language::BrazilianPortuguese => vec![
                    $(($file, include_str!(concat!("../locales/pt-BR/", $file)))),+
                ],
            }
        }
    };
}

locale_files!(
    "app.ftl",
    "strip.ftl",
    "menu.ftl",
    "projects.ftl",
    "rows.ftl",
    "drums.ftl",
    "song.ftl",
    "bars.ftl",
    "patterns.ftl",
    "tempo.ftl",
    "sounds.ftl",
    "bank.ftl",
    "errors.ftl",
);

type Bundle = FluentBundle<FluentResource>;

fn bundle(language: Language) -> &'static Bundle {
    static BUNDLES: OnceLock<Vec<Bundle>> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| Language::ALL.into_iter().map(build).collect());
    &bundles[language.index()]
}

fn build(language: Language) -> Bundle {
    let id: LanguageIdentifier = language.tag().parse().expect("language tags are valid");
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    // The marks Fluent puts around each argument for right-to-left text show as boxes in egui.
    bundle.set_use_isolating(false);
    if language == Language::BrazilianPortuguese {
        bundle.set_formatter(Some(decimal_comma));
    }
    bundle
        .add_builtins()
        .expect("the built-in functions are added once");
    for (file, text) in resources(language) {
        let resource =
            FluentResource::try_new(text.to_owned()).unwrap_or_else(|(partial, errors)| {
                log::error!("{}/{file} has errors: {errors:?}", language.tag());
                partial
            });
        if let Err(errors) = bundle.add_resource(resource) {
            log::error!("{}/{file} repeats messages: {errors:?}", language.tag());
        }
    }
    bundle
}

/// Writes numbers with a decimal comma, as Portuguese does. The numbers shown are small, so no
/// thousands separator is needed.
fn decimal_comma<M>(value: &FluentValue, _: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    Some(number.as_string().replace('.', ","))
}

/// The message `id` in the current language, with `args` filled in. Falls back to English, and
/// then to the id itself, so a missing translation shows something rather than nothing.
pub(crate) fn tr_args(id: &str, args: Option<&FluentArgs>) -> String {
    tr_in(language(), id, args)
}

pub(crate) fn tr_in(wanted: Language, id: &str, args: Option<&FluentArgs>) -> String {
    for language in [wanted, Language::English] {
        let bundle = bundle(language);
        let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) else {
            continue;
        };
        let mut errors = Vec::new();
        let text = bundle.format_pattern(pattern, args, &mut errors);
        if !errors.is_empty() {
            log::warn!("message {id} in {}: {errors:?}", language.tag());
        }
        return text.into_owned();
    }
    log::warn!("no message {id}");
    id.to_owned()
}

/// The message `id` in the current language.
pub(crate) fn tr(id: &str) -> String {
    tr_args(id, None)
}

/// The message with this id in the current language, as a `String`:
///
/// ```ignore
/// tr!("menu-undo")
/// tr!("bars-count", count = bars)
/// tr!("projects-saved", name = name.as_str())
/// ```
///
/// Argument names are written in snake_case and reach Fluent unchanged (`$count`, `$name`).
/// Numbers stay numbers, so Fluent can choose plural forms by them.
macro_rules! tr {
    ($id:literal) => {
        $crate::i18n::tr($id)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $( args.set(stringify!($name), $value); )+
        $crate::i18n::tr_args($id, Some(&args))
    }};
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
