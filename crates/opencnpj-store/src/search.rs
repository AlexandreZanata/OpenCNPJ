//! Query execution over the inverted index.
//!
//! * Text: every query token must match (AND). A token matches `razao_social` or
//!   `nome_fantasia` tokens; the last token also matches as a prefix (bounded expansion);
//!   optional fuzzy matching (edit distance 1, bounded) applies only to tokens with no exact
//!   match.
//! * Filters: values of one field are OR-ed, fields are AND-ed.
//! * Ranking (text queries): documents whose shortest indexed name has fewer tokens come
//!   first, active establishments before others, then CNPJ order. This is exact top-k over
//!   the whole candidate set, computed with per-bucket bitmap intersections.

use std::sync::Arc;

use roaring::{MultiOps, RoaringBitmap};

use crate::Error;
use crate::postings::{Field, MAX_NAME_LEN_BUCKET};
use crate::reader::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextField {
    #[default]
    Any,
    Razao,
    Fantasia,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub text: Option<String>,
    pub text_field: TextField,
    pub filters: Vec<(Field, Vec<String>)>,
    pub fuzzy: bool,
    pub offset: usize,
    pub limit: usize,
    /// Return documents with id greater than this (filter-only queries, keyset pagination).
    pub after: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Maximum terms a prefix expands to.
    pub max_prefix_terms: usize,
    /// Maximum terms a fuzzy token expands to.
    pub max_fuzzy_terms: usize,
    pub max_tokens: usize,
    /// Maximum dictionary terms examined per prefix/fuzzy token (bounds the FST walk).
    pub max_scan_terms: usize,
    /// Encoded postings bytes one expanded token may union (most frequent terms first).
    pub max_expansion_bytes: u64,
}

impl Limits {
    pub const DEFAULT: Limits =
        Limits { max_prefix_terms: 128, max_fuzzy_terms: 32, max_tokens: 8, max_scan_terms: 20_000, max_expansion_bytes: 8 << 20 };
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub total: u64,
    pub docs: Vec<u32>,
    /// A prefix or fuzzy expansion hit its limit; results may be incomplete.
    pub expansion_truncated: bool,
    pub next_after: Option<u32>,
}

/// Normalized value used in filter terms (codes are matched verbatim, `uf` uppercased).
pub fn filter_value(field: Field, v: &str) -> String {
    let v = v.trim();
    match field {
        Field::Uf | Field::Simples | Field::Mei => v.to_ascii_uppercase(),
        Field::Cnae | Field::CnaeSecundaria => v.chars().filter(char::is_ascii_digit).collect(),
        _ => v.to_string(),
    }
}

/// Candidate sets of a query.
#[derive(Debug, Default)]
pub struct Candidates {
    /// All matching documents (text and filters), `None` when the query has neither.
    pub all: Option<RoaringBitmap>,
    /// Documents where every token matched `razao_social` / `nome_fantasia` alone
    /// (restricted to `all`); used for ranking.
    pub by_razao: RoaringBitmap,
    pub by_fantasia: RoaringBitmap,
    pub has_text: bool,
    pub truncated: bool,
}

/// Postings of one token in one field, restricted to `restrict` (documents already
/// required by other tokens and filters) when given. Restricting each expanded term before
/// the union avoids materializing huge completions (`MAR` → MARIA, MARTINS, ...).
#[allow(clippy::too_many_arguments)]
fn token_postings(
    snap: &Snapshot,
    f: Field,
    tok: &str,
    prefix: bool,
    fuzzy: bool,
    restrict: Option<&RoaringBitmap>,
    limits: &Limits,
    truncated: &mut bool,
) -> Result<RoaringBitmap, Error> {
    let mut parts: Vec<Arc<RoaringBitmap>> = Vec::new();
    if prefix {
        let (terms, cut) =
            snap.prefix_terms_ranked(f, tok.as_bytes(), limits.max_prefix_terms, limits.max_scan_terms, limits.max_expansion_bytes);
        *truncated |= cut;
        for (_, off) in &terms {
            parts.push(snap.postings_by_offset(*off)?);
        }
    } else if let Some(p) = snap.postings(f, tok.as_bytes())? {
        parts.push(p);
    }
    // Fuzzy adds near terms even when the token exists: a typo is often a rare real word.
    if fuzzy && tok.len() >= 4 {
        let dist = if tok.len() >= 8 { 2 } else { 1 };
        let (terms, cut) =
            snap.fuzzy_terms_ranked(f, tok, dist, limits.max_fuzzy_terms, limits.max_scan_terms, limits.max_expansion_bytes)?;
        *truncated |= cut;
        for (_, off) in &terms {
            parts.push(snap.postings_by_offset(*off)?);
        }
    }
    Ok(match (restrict, parts.len()) {
        (_, 0) => RoaringBitmap::new(),
        (Some(r), _) => parts.iter().map(|p| r & &**p).union(),
        (None, 1) => Arc::unwrap_or_clone(parts.pop().expect("one part")),
        (None, _) => parts.iter().map(|p| &**p).union(),
    })
}

fn intersect_all(mut sets: Vec<RoaringBitmap>) -> Option<RoaringBitmap> {
    sets.sort_by_key(RoaringBitmap::len);
    let mut it = sets.into_iter();
    let mut acc = it.next()?;
    for s in it {
        if acc.is_empty() {
            break;
        }
        acc &= s;
    }
    Some(acc)
}

pub fn candidates(snap: &Snapshot, q: &Query, limits: &Limits) -> Result<Candidates, Error> {
    let mut out = Candidates::default();
    let mut sets: Vec<RoaringBitmap> = Vec::new();
    for (field, values) in &q.filters {
        let mut set = RoaringBitmap::new();
        for v in values {
            if let Some(p) = snap.postings(*field, filter_value(*field, v).as_bytes())? {
                set |= &*p;
            }
        }
        sets.push(set);
    }
    let filters = intersect_all(sets);
    if let Some(text) = q.text.as_deref() {
        out.has_text = true;
        let mut tokens = opencnpj_core::text::tokens(text);
        if tokens.len() > limits.max_tokens {
            // Long names: the first tokens are already highly selective.
            tokens.truncate(limits.max_tokens);
            out.truncated = true;
        }
        if tokens.is_empty() {
            out.all = Some(RoaringBitmap::new());
            return Ok(out);
        }
        let prefix_last = !text.ends_with(' ') && !out.truncated;
        let fields: &[Field] = match q.text_field {
            TextField::Any => &[Field::Razao, Field::Fantasia],
            TextField::Razao => &[Field::Razao],
            TextField::Fantasia => &[Field::Fantasia],
        };
        // Exact tokens first: their intersection (with the filters) restricts the
        // expensive prefix/fuzzy expansions that follow.
        let expands = |i: usize| q.fuzzy || (prefix_last && i + 1 == tokens.len());
        let mut order: Vec<usize> = (0..tokens.len()).collect();
        order.sort_by_key(|&i| expands(i));
        // per_field[f]: postings of each token in field f.
        let mut per_field: Vec<Vec<RoaringBitmap>> = vec![Vec::new(); fields.len()];
        let mut any: Vec<RoaringBitmap> = Vec::new();
        let mut restrict: Option<Option<RoaringBitmap>> = None;
        for i in order {
            let tok = &tokens[i];
            let prefix = prefix_last && i + 1 == tokens.len();
            let r = if expands(i) {
                // Computed once, before the first expansion (exact tokens come first).
                if restrict.is_none() || q.fuzzy {
                    let mut sets: Vec<&RoaringBitmap> = any.iter().chain(filters.as_ref()).collect();
                    sets.sort_by_key(|b| b.len());
                    restrict = Some(sets.split_first().map(|(first, rest)| {
                        let mut acc = (*first).clone();
                        for s in rest {
                            acc &= *s;
                        }
                        acc
                    }));
                }
                restrict.as_ref().and_then(Option::as_ref)
            } else {
                None
            };
            let mut parts = Vec::with_capacity(fields.len());
            for &f in fields {
                parts.push(token_postings(snap, f, tok, prefix, q.fuzzy, r, limits, &mut out.truncated)?);
            }
            let union = parts.iter().union();
            for (fi, p) in parts.into_iter().enumerate() {
                per_field[fi].push(p);
            }
            if union.is_empty() {
                out.all = Some(union);
                return Ok(out);
            }
            any.push(union);
        }
        let mut all = intersect_all(any).expect("tokens");
        if let Some(f) = &filters {
            all &= f;
        }
        for (fi, &f) in fields.iter().enumerate() {
            let mut set = intersect_all(std::mem::take(&mut per_field[fi])).expect("tokens");
            set &= &all;
            match f {
                Field::Razao => out.by_razao = set,
                _ => out.by_fantasia = set,
            }
        }
        out.all = Some(all);
    } else {
        out.all = filters;
    }
    Ok(out)
}

pub fn search(snap: &Snapshot, q: &Query, limits: &Limits) -> Result<SearchResult, Error> {
    let c = candidates(snap, q, limits)?;
    let set = c.all.ok_or_else(|| Error::Invalid("a text query or at least one filter is required".into()))?;
    let total = set.len();
    let want = q.offset.saturating_add(q.limit);
    let mut docs = Vec::with_capacity(want.min(10_000));
    let mut next_after = None;
    if c.has_text {
        // Shortest matching name first; within a bucket, active establishments first, then
        // CNPJ order. Documents matched only across both fields come last.
        let active = snap.postings(Field::Situacao, b"02")?.unwrap_or_default();
        let active = &*active;
        let mut emitted = RoaringBitmap::new();
        'outer: for len in (1..=MAX_NAME_LEN_BUCKET).chain(std::iter::once(0)) {
            let v = len.to_string();
            let mut bucket = RoaringBitmap::new();
            if !c.by_razao.is_empty()
                && let Some(p) = snap.postings(Field::RazaoLen, v.as_bytes())?
            {
                bucket |= &c.by_razao & &*p;
            }
            if !c.by_fantasia.is_empty()
                && let Some(p) = snap.postings(Field::FantasiaLen, v.as_bytes())?
            {
                bucket |= &c.by_fantasia & &*p;
            }
            bucket -= &emitted;
            if bucket.is_empty() {
                continue;
            }
            emitted |= &bucket;
            let act = &bucket & active;
            let inact = &bucket - &act;
            for part in [act, inact] {
                for d in part.iter() {
                    if docs.len() >= want {
                        break 'outer;
                    }
                    docs.push(d);
                }
            }
        }
        if docs.len() < want {
            let rest = &set - &emitted;
            docs.extend(rest.iter().take(want - docs.len()));
        }
        docs.drain(..q.offset.min(docs.len()));
    } else {
        let mut set = set;
        if let Some(after) = q.after {
            set.remove_range(..=after);
        }
        docs.extend(set.iter().skip(q.offset).take(q.limit));
        if docs.len() == q.limit {
            next_after = docs.last().copied();
        }
    }
    Ok(SearchResult { total, docs, expansion_truncated: c.truncated, next_after })
}

/// Counts documents per value of `group_by`, restricted to the query's candidates.
///
/// High-cardinality fields (município, CNAE, natureza) need a narrowing filter: each value
/// is one postings list to decode. Heavy analytics belong in a columnar engine (export to
/// Parquet/DuckDB), not in the request path.
pub fn group_counts(snap: &Snapshot, q: &Query, group_by: Field, limits: &Limits) -> Result<Vec<(String, u64)>, Error> {
    let set = candidates(snap, q, limits)?.all;
    let high_cardinality = matches!(group_by, Field::Municipio | Field::Cnae | Field::CnaeSecundaria | Field::Natureza);
    if high_cardinality && set.is_none() {
        return Err(Error::Invalid(format!("group_by={} requires q or a filter", group_by.name())));
    }
    let mut out = Vec::new();
    for (value, off) in snap.prefix_terms(group_by, b"", usize::MAX) {
        let p = snap.postings_by_offset(off)?;
        let n = match &set {
            Some(s) => s.intersection_len(&p),
            None => p.len(),
        };
        if n > 0 {
            out.push((String::from_utf8_lossy(&value).into_owned(), n));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(out)
}
