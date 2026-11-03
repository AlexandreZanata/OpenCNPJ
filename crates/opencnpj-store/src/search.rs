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
use crate::numeric::{NumField, Range};
use crate::postings::{Field, MAX_NAME_LEN_BUCKET};
use crate::reader::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextField {
    #[default]
    Any,
    Razao,
    Fantasia,
}

/// Range (or "no value") condition on a numeric column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeFilter {
    pub field: NumField,
    /// `None`: documents without a value.
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub text: Option<String>,
    pub text_field: TextField,
    /// Term filters: values of one entry are OR-ed, entries AND-ed. Values accept prefixes
    /// (`47*`, short CNAE/CEP/natureza codes) and CNAE section letters (`G`).
    pub filters: Vec<(Field, Vec<String>)>,
    /// CNPJ prefixes (OR-ed), 1–14 characters.
    pub cnpj_prefixes: Vec<String>,
    /// Numeric conditions (AND-ed).
    pub ranges: Vec<RangeFilter>,
    /// Order by a numeric column instead of relevance/CNPJ (`true` = descending).
    pub sort: Option<(NumField, bool)>,
    pub fuzzy: bool,
    /// Retry with fuzzy matching when the exact query finds nothing.
    pub fuzzy_auto: bool,
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
    /// The automatic fuzzy retry produced these results.
    pub fuzzy_used: bool,
}

/// Normalized value used in filter terms (codes are matched verbatim, `uf` uppercased).
pub fn filter_value(field: Field, v: &str) -> String {
    let v = v.trim();
    match field {
        Field::Uf | Field::Simples | Field::Mei => v.to_ascii_uppercase(),
        Field::Cnae | Field::CnaeSecundaria | Field::CnaeQualquer | Field::Cep | Field::Cpf | Field::CpfQualquer => digits(v),
        Field::Ddd => digits(v).trim_start_matches('0').to_string(),
        Field::Contato => v.to_ascii_lowercase(),
        _ => v.to_string(),
    }
}

fn digits(v: &str) -> String {
    v.chars().filter(char::is_ascii_digit).collect()
}

/// CNAE 2.x sections → division (2-digit) prefixes.
fn cnae_section(letter: char) -> Option<Vec<String>> {
    let (a, b) = match letter.to_ascii_uppercase() {
        'A' => (1, 3),
        'B' => (5, 9),
        'C' => (10, 33),
        'D' => (35, 35),
        'E' => (36, 39),
        'F' => (41, 43),
        'G' => (45, 47),
        'H' => (49, 53),
        'I' => (55, 56),
        'J' => (58, 63),
        'K' => (64, 66),
        'L' => (68, 68),
        'M' => (69, 75),
        'N' => (77, 82),
        'O' => (84, 84),
        'P' => (85, 85),
        'Q' => (86, 88),
        'R' => (90, 93),
        'S' => (94, 96),
        'T' => (97, 97),
        'U' => (99, 99),
        _ => return None,
    };
    Some((a..=b).map(|d| format!("{d:02}")).collect())
}

/// How one filter value is looked up: exact terms and/or term prefixes, per stored field.
#[derive(Debug, Default, PartialEq)]
pub struct Pattern {
    pub exact: Vec<(Field, String)>,
    pub prefixes: Vec<(Field, String)>,
}

/// Interprets a filter value: CNAE sections and partial codes, CEP/natureza prefixes, `*`
/// suffixes, CPF/partner document normalization, and the query-only `cnae_qualquer`.
pub fn filter_pattern(field: Field, v: &str) -> Result<Pattern, Error> {
    let mut p = Pattern::default();
    let raw = v.trim();
    let star = raw.ends_with('*');
    let base = raw.trim_end_matches('*');
    let stored: &[Field] = match field {
        Field::CnaeQualquer => &[Field::Cnae, Field::CnaeSecundaria],
        Field::CpfQualquer => &[Field::Cpf, Field::SocioDoc],
        f => std::slice::from_ref(match f {
            Field::Cnae => &Field::Cnae,
            Field::CnaeSecundaria => &Field::CnaeSecundaria,
            _ => &field,
        }),
    };
    for &f in stored {
        match f {
            Field::Cnae | Field::CnaeSecundaria => {
                if base.len() == 1 && base.chars().all(|c| c.is_ascii_alphabetic()) {
                    let divs = cnae_section(base.chars().next().expect("one char"))
                        .ok_or_else(|| Error::Invalid(format!("unknown CNAE section {base}")))?;
                    p.prefixes.extend(divs.into_iter().map(|d| (f, d)));
                    continue;
                }
                let d = digits(base);
                if d.is_empty() || d.len() > 7 {
                    return Err(Error::Invalid(format!("bad CNAE {raw}")));
                }
                if d.len() == 7 && !star { p.exact.push((f, d)) } else { p.prefixes.push((f, d)) }
            }
            Field::Cep | Field::Natureza => {
                let d = digits(base);
                let full = if f == Field::Cep { 8 } else { 4 };
                if d.is_empty() || d.len() > full {
                    return Err(Error::Invalid(format!("bad {} {raw}", f.name())));
                }
                if d.len() == full && !star { p.exact.push((f, d)) } else { p.prefixes.push((f, d)) }
            }
            // `cpf` (query field) = the CPF in MEI/EI names, plus partners whose masked CPF
            // shows the same 6 middle digits (an approximate match, by construction).
            Field::Cpf => {
                let d = digits(base);
                if d.len() != 11 {
                    return Err(Error::Invalid("cpf must have 11 digits".into()));
                }
                p.exact.push((Field::Cpf, d));
            }
            Field::SocioDoc => {
                let d = digits(base);
                match d.len() {
                    14 => p.exact.push((f, d)),
                    11 => p.exact.push((f, format!("*{}", &d[3..9]))),
                    6 => p.exact.push((f, format!("*{d}"))),
                    _ => {
                        return Err(Error::Invalid(
                            "socio document must be a 14-digit CNPJ, an 11-digit CPF or 6 visible CPF digits".into(),
                        ));
                    }
                }
            }
            _ => {
                let val = filter_value(f, base);
                if star { p.prefixes.push((f, val)) } else { p.exact.push((f, val)) }
            }
        }
    }
    Ok(p)
}

/// Maximum dictionary terms one prefix filter may expand to.
pub const MAX_FILTER_TERMS: usize = 500_000;

/// Postings lists whose union is the documents matching any of `values` in `field`.
fn filter_parts(snap: &Snapshot, field: Field, values: &[String]) -> Result<Vec<Arc<RoaringBitmap>>, Error> {
    let mut parts: Vec<Arc<RoaringBitmap>> = Vec::new();
    for v in values {
        let pat = filter_pattern(field, v)?;
        for (f, t) in &pat.exact {
            if let Some(p) = snap.postings(*f, t.as_bytes())? {
                parts.push(p);
            }
        }
        for (f, t) in &pat.prefixes {
            for (_, off) in snap.prefix_terms(*f, t.as_bytes(), MAX_FILTER_TERMS) {
                parts.push(snap.postings_by_offset(off)?);
            }
        }
    }
    Ok(parts)
}

/// Intersection of every filter, evaluated smallest first. When the running intersection
/// is much smaller than a later filter, that filter's postings are restricted to it before
/// their union, so a broad prefix filter (a CNAE division, a CEP prefix) does not
/// materialize its full union.
fn filters_intersection(snap: &Snapshot, filters: &[(Field, Vec<String>)]) -> Result<Option<RoaringBitmap>, Error> {
    let mut groups: Vec<(u64, Vec<Arc<RoaringBitmap>>)> = Vec::with_capacity(filters.len());
    for (field, values) in filters {
        let parts = filter_parts(snap, *field, values)?;
        let size = parts.iter().map(|p| p.len()).sum();
        groups.push((size, parts));
    }
    groups.sort_by_key(|g| g.0);
    let mut acc: Option<RoaringBitmap> = None;
    for (size, parts) in groups {
        let next = match &acc {
            None => parts.iter().map(|p| &**p).union(),
            Some(a) if a.is_empty() => return Ok(acc),
            // Restricting each part pays off only when the running set is much smaller.
            Some(a) if a.len().saturating_mul(8) < size => parts.iter().map(|p| a & &**p).union(),
            Some(a) => a & parts.iter().map(|p| &**p).union(),
        };
        acc = Some(next);
    }
    Ok(acc)
}

/// A text query that is only a CNPJ (or a CNPJ prefix of at least 8 digits, other than
/// 11) is treated as a CNPJ prefix search.
fn cnpj_like(text: &str) -> Option<String> {
    let t = text.trim();
    let ok = t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '/' | '-' | ' '));
    let d = digits(t);
    // Exactly 11 digits is a CPF, not a CNPJ prefix.
    (ok && (8..=14).contains(&d.len()) && d.len() != 11).then_some(d)
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
        // Swapped adjacent letters (Damerau transpositions) cost 2 Levenshtein edits but
        // are the most common typo: look them up exactly.
        let b = tok.as_bytes();
        for i in 0..b.len().saturating_sub(1) {
            if b[i] != b[i + 1] {
                let mut t = b.to_vec();
                t.swap(i, i + 1);
                if let Some(p) = snap.postings(f, &t)? {
                    parts.push(p);
                }
            }
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
    if let Some(f) = filters_intersection(snap, &q.filters)? {
        sets.push(f);
    }
    let mut text = q.text.as_deref();
    let mut prefixes: Vec<&str> = q.cnpj_prefixes.iter().map(String::as_str).collect();
    let cnpj_text = text.and_then(cnpj_like);
    if let Some(c) = &cnpj_text {
        prefixes.push(c);
        text = None;
    }
    if !prefixes.is_empty() {
        let mut set = RoaringBitmap::new();
        for p in prefixes {
            set |= snap.cnpj_prefix_docs(p)?;
        }
        sets.push(set);
    }
    // 11-digit tokens in a name query are CPFs (MEI/EI owners): exact `cpf` matches.
    let mut tokens = text.map(opencnpj_core::text::tokens).unwrap_or_default();
    for cpf in tokens.iter().filter(|t| !opencnpj_core::text::is_indexed_token(t)) {
        sets.push(snap.postings(Field::Cpf, cpf.as_bytes())?.map(Arc::unwrap_or_clone).unwrap_or_default());
    }
    tokens.retain(|t| opencnpj_core::text::is_indexed_token(t));
    let mut filters = intersect_all(sets);
    for r in &q.ranges {
        let col = snap
            .numeric(r.field)
            .ok_or_else(|| Error::Invalid(format!("snapshot has no {} column (format 1.2+ needed)", r.field.name())))?;
        filters = Some(col.select(snap.bytes(), r.range, filters.as_ref()));
    }
    if let Some(text) = text.filter(|_| !tokens.is_empty()) {
        out.has_text = true;
        if tokens.len() > limits.max_tokens {
            // Long names: the first tokens are already highly selective.
            tokens.truncate(limits.max_tokens);
            out.truncated = true;
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
    } else if text.is_some_and(|t| !t.trim().is_empty()) && filters.is_none() {
        // Only stopwords or CPF tokens without matches.
        out.has_text = true;
        out.all = Some(RoaringBitmap::new());
    } else {
        out.all = filters;
    }
    Ok(out)
}

/// `fuzzy=auto` retries with typo tolerance below this many exact hits.
pub const AUTO_FUZZY_BELOW: u64 = 5;

pub fn search(snap: &Snapshot, q: &Query, limits: &Limits) -> Result<SearchResult, Error> {
    let r = search_once(snap, q, limits)?;
    // Few or no hits: retry with typo tolerance. Fuzzy matching is a superset of exact
    // matching, so its results replace the exact ones only when it finds more.
    if r.total < AUTO_FUZZY_BELOW && q.fuzzy_auto && !q.fuzzy && q.text.is_some() {
        let mut fq = q.clone();
        fq.fuzzy = true;
        let mut f = search_once(snap, &fq, limits)?;
        if f.total > r.total {
            f.fuzzy_used = true;
            return Ok(f);
        }
    }
    Ok(r)
}

fn search_once(snap: &Snapshot, q: &Query, limits: &Limits) -> Result<SearchResult, Error> {
    let c = candidates(snap, q, limits)?;
    let set = c.all.ok_or_else(|| Error::Invalid("a text query or at least one filter is required".into()))?;
    let total = set.len();
    let want = q.offset.saturating_add(q.limit);
    let mut docs = Vec::with_capacity(want.min(10_000));
    let mut next_after = None;
    if let Some((field, desc)) = q.sort {
        let col = snap.numeric(field).ok_or_else(|| Error::Invalid(format!("snapshot has no {} column", field.name())))?;
        docs = col.top_k(snap.bytes(), &set, want, desc);
        docs.drain(..q.offset.min(docs.len()));
    } else if c.has_text {
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
    Ok(SearchResult { total, docs, expansion_truncated: c.truncated, next_after, fuzzy_used: false })
}

/// Counts documents per value of `group_by`, restricted to the query's candidates.
///
/// High-cardinality fields (município, CNAE, natureza) need a narrowing filter. Município
/// and CNAE principal are counted in one pass over the candidates when the snapshot has
/// forward columns (format ≥ 1.1); otherwise, and for the other fields, each value is one
/// postings list to intersect. Heavy analytics belong in a columnar engine (export to
/// Parquet/DuckDB), not in the request path.
pub fn group_counts(snap: &Snapshot, q: &Query, group_by: Field, limits: &Limits) -> Result<Vec<(String, u64)>, Error> {
    let set = candidates(snap, q, limits)?.all;
    let high_cardinality = matches!(group_by, Field::Municipio | Field::Cnae | Field::CnaeSecundaria | Field::Natureza);
    if high_cardinality && set.is_none() {
        return Err(Error::Invalid(format!("group_by={} requires q or a filter", group_by.name())));
    }
    let mut out = match (&set, snap.doc_values(group_by)) {
        (Some(s), Some(col)) => {
            let counts = snap.doc_value_counts(col, s.iter())?;
            col.values.iter().zip(&counts[1..]).filter(|(_, n)| **n > 0).map(|(v, n)| (v.clone(), *n)).collect()
        }
        _ => group_counts_postings(snap, set.as_ref(), group_by)?,
    };
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(out)
}

/// Per-value postings intersection (unsorted); the reference path for every field.
pub fn group_counts_postings(snap: &Snapshot, set: Option<&RoaringBitmap>, group_by: Field) -> Result<Vec<(String, u64)>, Error> {
    let mut out = Vec::new();
    for (value, off) in snap.prefix_terms(group_by, b"", usize::MAX) {
        let p = snap.postings_by_offset(off)?;
        let n = match set {
            Some(s) => s.intersection_len(&p),
            None => p.len(),
        };
        if n > 0 {
            out.push((String::from_utf8_lossy(&value).into_owned(), n));
        }
    }
    Ok(out)
}
