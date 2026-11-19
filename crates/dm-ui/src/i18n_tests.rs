use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fluent_syntax::ast;

use super::*;

/// Every message and term of a language, each with the variables it uses, by id. Attributes
/// count as `id.attribute`.
fn messages(language: Language) -> BTreeMap<String, BTreeSet<String>> {
    let mut all = BTreeMap::new();
    for (file, text) in resources(language) {
        let resource = fluent_syntax::parser::parse(text)
            .unwrap_or_else(|(_, errors)| panic!("{}/{file}: {errors:?}", language.tag()));
        for entry in resource.body {
            let (id, value, attributes) = match entry {
                ast::Entry::Message(m) => (m.id.name.to_owned(), m.value, m.attributes),
                ast::Entry::Term(t) => (format!("-{}", t.id.name), Some(t.value), t.attributes),
                _ => continue,
            };
            let mut variables = BTreeSet::new();
            if let Some(pattern) = &value {
                pattern_variables(pattern, &mut variables);
            }
            let fresh = all.insert(id.clone(), variables).is_none();
            assert!(fresh, "{}/{file} repeats {id}", language.tag());
            for attribute in attributes {
                let mut variables = BTreeSet::new();
                pattern_variables(&attribute.value, &mut variables);
                all.insert(format!("{id}.{}", attribute.id.name), variables);
            }
        }
    }
    all
}

fn pattern_variables(pattern: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_variables(expression, out);
        }
    }
}

fn expression_variables(expression: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => inline_variables(inline, out),
        ast::Expression::Select { selector, variants } => {
            inline_variables(selector, out);
            for variant in variants {
                pattern_variables(&variant.value, out);
            }
        }
    }
}

fn inline_variables(inline: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                inline_variables(argument, out);
            }
        }
        ast::InlineExpression::Placeable { expression } => expression_variables(expression, out),
        _ => {}
    }
}

#[test]
fn every_language_has_the_same_messages_and_variables() {
    let english = messages(Language::English);
    assert!(!english.is_empty());
    for language in Language::ALL {
        let other = messages(language);
        let missing: Vec<_> = english.keys().filter(|k| !other.contains_key(*k)).collect();
        let extra: Vec<_> = other.keys().filter(|k| !english.contains_key(*k)).collect();
        assert!(missing.is_empty(), "{} lacks {missing:?}", language.tag());
        assert!(extra.is_empty(), "{} has unknown {extra:?}", language.tag());
        for (id, variables) in &english {
            // Terms are private to a language, so they may differ.
            if !id.starts_with('-') {
                assert_eq!(&other[id], variables, "{} {id} variables", language.tag());
            }
        }
    }
}

#[test]
fn every_message_loads_into_its_bundle() {
    for language in Language::ALL {
        let bundle = build(language);
        for id in messages(language).keys() {
            if !id.starts_with('-') && !id.contains('.') {
                assert!(bundle.has_message(id), "{} {id}", language.tag());
            }
        }
    }
}

/// The Rust files under `dir`, as text, except this module's own, whose examples and tests name
/// messages that need not exist.
fn sources(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs")
            && !path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("i18n")
        {
            out.push(std::fs::read_to_string(&path).unwrap());
        }
    }
}

/// The ids of every `tr!("…")` and `tr("…")` in `text`.
fn asked_for(text: &str, out: &mut BTreeSet<String>) {
    for start in ["tr!(\"", "tr(\""] {
        for (at, _) in text.match_indices(start) {
            // Not the end of a longer name, such as `jni_str!("`.
            let before = text[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let rest = &text[at + start.len()..];
            out.insert(rest[..rest.find('"').unwrap()].to_owned());
        }
    }
}

/// Ids built at run time, such as `drums-name-36`, start with one of these.
const BUILT_PREFIXES: [&str; 1] = ["drums-name-"];

#[test]
fn every_message_the_code_asks_for_exists_and_every_message_is_used() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut texts = Vec::new();
    sources(&root.join("src"), &mut texts);
    // The hosts name some messages too, such as where exports go.
    sources(&root.join("../../apps"), &mut texts);
    let mut asked = BTreeSet::new();
    for text in &texts {
        asked_for(text, &mut asked);
    }
    let english = messages(Language::English);
    let missing: Vec<_> = asked
        .iter()
        .filter(|id| !english.contains_key(*id))
        .collect();
    assert!(missing.is_empty(), "no message for {missing:?}");
    // A message counts as used when its id appears in quotes anywhere, so ids chosen in a
    // `match` and passed to `tr` count too.
    let unused: Vec<_> = english
        .keys()
        .filter(|id| !id.starts_with('-') && !id.contains('.'))
        .filter(|id| !BUILT_PREFIXES.iter().any(|p| id.starts_with(p)))
        .filter(|id| {
            let quoted = format!("\"{id}\"");
            !texts.iter().any(|t| t.contains(&quoted))
        })
        .collect();
    assert!(unused.is_empty(), "never shown: {unused:?}");
}

#[test]
fn system_tags_match_the_nearest_language() {
    assert_eq!(
        Language::matching("pt-BR"),
        Some(Language::BrazilianPortuguese)
    );
    assert_eq!(
        Language::matching("pt_PT"),
        Some(Language::BrazilianPortuguese)
    );
    assert_eq!(
        Language::matching("PT"),
        Some(Language::BrazilianPortuguese)
    );
    assert_eq!(Language::matching("en-GB"), Some(Language::English));
    assert_eq!(Language::matching("en_US.UTF-8"), Some(Language::English));
    assert_eq!(Language::matching("de-DE"), None);
    assert_eq!(Language::matching(""), None);
    for language in Language::ALL {
        assert_eq!(Language::from_tag(language.tag()), Some(language));
    }
}

#[test]
fn text_follows_the_language_and_decimals_use_its_separator() {
    let portuguese = tr_in(
        Language::BrazilianPortuguese,
        "app-export-place-iphone",
        None,
    );
    let english = tr_in(Language::English, "app-export-place-iphone", None);
    assert_ne!(portuguese, english);
    assert_eq!(
        decimal_comma(&FluentValue::from(1.5), &()).as_deref(),
        Some("1,5")
    );
    assert_eq!(
        tr_in(Language::English, "no-such-message", None),
        "no-such-message"
    );
}
