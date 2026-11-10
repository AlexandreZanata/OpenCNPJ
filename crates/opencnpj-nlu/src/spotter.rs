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

    /// Typo recovery only after a geographic preposition and never for a short token.
    pub fn fuzzy_place(&self, word: &str) -> Option<Vec<Entity>> {
        if word.len() < 5 || !word.is_ascii() {
            return None;
        }
        let mut candidates = Vec::new();
        for (key, &node) in &self.nodes[0].children {
            if one_edit(word.as_bytes(), key.as_bytes()) && self.nodes[node].children.is_empty() {
                let values: Vec<_> = self.nodes[node].values.iter().filter(|e| matches!(e, Entity::Municipality { .. })).cloned().collect();
                if !values.is_empty() {
                    candidates.push(values);
                }
                if candidates.len() > 1 {
                    return None;
                }
            }
        }
        candidates.pop()
    }

    /// A single-word activity one edit away from `word`, when exactly one exists.
    pub fn fuzzy_sector(&self, word: &str) -> Option<Vec<Entity>> {
        if word.len() < 7 || !word.is_ascii() {
            return None;
        }
        let mut candidates: Vec<Vec<Entity>> = Vec::new();
        for (key, &node) in &self.nodes[0].children {
            if key.len() >= 6 && one_edit(word.as_bytes(), key.as_bytes()) {
                let values: Vec<_> = self.nodes[node].values.iter().filter(|e| matches!(e, Entity::Sector(_))).cloned().collect();
                if !values.is_empty() && !candidates.contains(&values) {
                    candidates.push(values);
                }
            }
        }
        if candidates.len() == 1 { candidates.pop() } else { None }
    }
}

fn one_edit(a: &[u8], b: &[u8]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    if a.len() == b.len() {
        let d: Vec<_> = a.iter().zip(b).enumerate().filter_map(|(i, (a, b))| (a != b).then_some(i)).collect();
        return d.len() == 1 || d.len() == 2 && d[1] == d[0] + 1 && a[d[0]] == b[d[1]] && a[d[1]] == b[d[0]];
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
    fn edits_are_bounded() {
        assert!(one_edit(b"sorrisso", b"sorriso"));
        assert!(one_edit(b"sinpo", b"sinop"));
        assert!(!one_edit(b"soxeiso", b"sorriso"));
    }
}
