//! Closed vocabulary of the grammar.
//!
//! A question is executed only when every word outside a catalogue entity is known here.
//! An unknown word ("empresas de energia solar") names something the grammar cannot apply,
//! and dropping it would answer a broader question than the one asked.

use crate::normalize;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Known {
    search: BTreeSet<String>,
    general: BTreeSet<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    /// Word lists by role; the roles document the file and are not used by the grammar.
    search: BTreeMap<String, Vec<String>>,
    general: BTreeMap<String, Vec<String>>,
}

fn words_of(lists: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    lists.values().flatten().flat_map(|phrase| normalize::words(phrase)).collect()
}

/// Numbers, codes and single letters are checked by the rules that read them.
fn literal(word: &str) -> bool {
    word.chars().count() == 1 || word.chars().any(|c| c.is_ascii_digit())
}

impl Known {
    pub(crate) fn embedded() -> Self {
        let file: File = toml::from_str(include_str!("../lang/pt-BR/vocabulary.toml")).expect("embedded vocabulary");
        let mut search = words_of(&file.search);
        // A lowercase state acronym that is not a state there ("se", "para") is a function word.
        search.extend(crate::lang::UFS.iter().map(|uf| uf.to_ascii_lowercase()));
        Self { search, general: words_of(&file.general) }
    }

    /// Words of any question the grammar may execute.
    pub(crate) fn knows(&self, word: &str) -> bool {
        literal(word) || self.search.contains(word) || self.general.contains(word)
    }

    /// Words a search-style noun phrase may hold: each is read by a filter or the structure.
    pub(crate) fn searches(&self, word: &str) -> bool {
        literal(word) || self.search.contains(word)
    }
}
