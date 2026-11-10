//! Immutable longest-match catalogue trie with bounded, unique typo recovery.

use crate::normalize::words;
use opencnpj_query::ir::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entity {
    Municipality { id: MunicipalityId, uf: UfId },
    Uf(UfId),
    Region(RegionId),
    Sector(SectorId),
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, usize>,
    values: Vec<Entity>,
}

pub struct Lexicon {
    nodes: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub values: Vec<Entity>,
    pub fuzzy: bool,
}

impl Default for Lexicon {
    fn default() -> Self {
        Self { nodes: vec![Node::default()] }
    }
}

impl Lexicon {
    pub fn insert(&mut self, phrase: &str, entity: Entity) {
        let mut node = 0;
        for word in words(phrase) {
            node = if let Some(&next) = self.nodes[node].children.get(&word) {
                next
            } else {
                let next = self.nodes.len();
                self.nodes.push(Node::default());
                self.nodes[node].children.insert(word, next);
                next
            };
        }
        if node != 0 && !self.nodes[node].values.contains(&entity) {
            self.nodes[node].values.push(entity);
        }
    }

    pub fn scan(&self, tokens: &[String]) -> Vec<Hit> {
        let mut hits = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let mut node = 0;
            let mut best = None;
            for (j, token) in tokens.iter().enumerate().skip(i).take(12) {
                let Some(&next) = self.nodes[node].children.get(token) else { break };
                node = next;
                if !self.nodes[node].values.is_empty() {
                    best = Some((j + 1, self.nodes[node].values.clone()));
                }
            }
            if let Some((end, values)) = best {
                hits.push(Hit { start: i, end, values, fuzzy: false });
                i = end;
            } else {
                i += 1;
            }
        }
        hits
    }

    /// A complete municipality name one edit away, optionally restricted to a supplied UF.
    pub(crate) fn fuzzy_place(&self, name: &[String], uf: Option<UfId>) -> Option<Vec<Entity>> {
        let minimum = if name.len() == 1 { 5 } else { 4 };
        self.fuzzy_entities(
            name,
            minimum,
            |_| true,
            |entity| matches!(entity, Entity::Municipality { uf: actual, .. } if uf.is_none_or(|expected| expected == *actual)),
        )
    }

    /// A complete activity alias one edit away, keeping the original entity boundary.
    pub(crate) fn fuzzy_sector(&self, name: &[String], editable: impl Fn(&str) -> bool) -> Option<Vec<Entity>> {
        let minimum = if name.len() == 1 { 6 } else { 4 };
        self.fuzzy_entities(name, minimum, editable, |entity| matches!(entity, Entity::Sector(_)))
    }

    fn fuzzy_entities(
        &self,
        name: &[String],
        minimum: usize,
        editable: impl Fn(&str) -> bool,
        keep: impl Fn(&Entity) -> bool,
    ) -> Option<Vec<Entity>> {
        let mut pending = vec![(0, 0, false)];
        let mut result = None;
        while let Some((node, depth, edited)) = pending.pop() {
            if let Some(word) = name.get(depth) {
                let children = &self.nodes[node].children;
                if let Some(&next) = children.get(word) {
                    pending.push((next, depth + 1, edited));
                }
                if !edited && word.len() >= minimum && word.is_ascii() && editable(word) {
                    pending.extend(children.iter().filter_map(|(key, &next)| {
                        (key.len() >= minimum && key.is_ascii() && one_edit(word.as_bytes(), key.as_bytes())).then_some((
                            next,
                            depth + 1,
                            true,
                        ))
                    }));
                }
            } else if edited {
                let values: Vec<_> = self.nodes[node].values.iter().filter(|entity| keep(entity)).cloned().collect();
                if !values.is_empty() {
                    if result.as_ref().is_some_and(|other| *other != values) {
                        return None;
                    }
                    result = Some(values);
                }
            }
        }
        result
    }
}

fn one_edit(a: &[u8], b: &[u8]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    if a.len() == b.len() {
        let Some(first) = a.iter().zip(b).position(|(left, right)| left != right) else { return false };
        return a[first + 1..] == b[first + 1..]
            || first + 1 < a.len() && a[first] == b[first + 1] && a[first + 1] == b[first] && a[first + 2..] == b[first + 2..];
    }
    let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
    let mut i = 0;
    while i < short.len() && short[i] == long[i] {
        i += 1;
    }
    short[i..] == long[i + 1..]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn longest_match_retains_homonyms() {
        let mut l = Lexicon::default();
        l.insert("Mato Grosso", Entity::Uf(UfId(12)));
        l.insert("Mato Grosso do Sul", Entity::Uf(UfId(11)));
        let h = l.scan(&words("em Mato Grosso do Sul"));
        assert_eq!(h[0].values, [Entity::Uf(UfId(11))]);
        l.insert("Bom Jesus", Entity::Municipality { id: MunicipalityId(1), uf: UfId(2) });
        l.insert("Bom Jesus", Entity::Municipality { id: MunicipalityId(3), uf: UfId(4) });
        assert_eq!(l.scan(&words("Bom Jesus"))[0].values.len(), 2);
    }
    #[test]
    fn compound_recovery_respects_the_uf_and_requires_one_edit() {
        let mut lexicon = Lexicon::default();
        let west = Entity::Municipality { id: MunicipalityId(1), uf: UfId(1) };
        let east = Entity::Municipality { id: MunicipalityId(2), uf: UfId(2) };
        lexicon.insert("Porto Alegre", west.clone());
        lexicon.insert("Porto Alegra", east.clone());
        assert_eq!(lexicon.fuzzy_place(&words("Porto Alegr"), None), None);
        assert_eq!(lexicon.fuzzy_place(&words("Porto Alegr"), Some(UfId(1))), Some(vec![west]));
        assert_eq!(lexicon.fuzzy_place(&words("Porto Alegr"), Some(UfId(3))), None);
        assert_eq!(lexicon.fuzzy_place(&words("Prota Alegr"), Some(UfId(2))), None);
    }

    #[test]
    fn edits_are_bounded() {
        assert!(one_edit(b"sorrisso", b"sorriso"));
        assert!(one_edit(b"sinpo", b"sinop"));
        assert!(!one_edit(b"soxeiso", b"sorriso"));
    }
}
