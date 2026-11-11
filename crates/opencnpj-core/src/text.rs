//! Source decoding (ISO-8859-1) and search normalization.
//!
//! Normalization is only used to build and query the text index. Stored values are
//! never normalized.

/// Decodes ISO-8859-1 bytes. Every byte maps to the code point of the same value, so the
/// conversion is total and reversible.
pub fn latin1_to_string(bytes: &[u8], out: &mut String) {
    if bytes.is_ascii() {
        out.push_str(std::str::from_utf8(bytes).expect("ascii is utf-8"));
    } else {
        out.extend(bytes.iter().map(|&b| char::from(b)));
    }
}

/// Folds a character to its uppercase ASCII search form, or `None` for separators.
#[inline]
fn fold(c: char) -> Option<char> {
    Some(match c {
        'a'..='z' => c.to_ascii_uppercase(),
        'A'..='Z' | '0'..='9' => c,
        'À'..='Å' | 'à'..='å' | 'ª' => 'A',
        'Ç' | 'ç' => 'C',
        'È'..='Ë' | 'è'..='ë' => 'E',
        'Ì'..='Ï' | 'ì'..='ï' => 'I',
        'Ñ' | 'ñ' => 'N',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'º' => 'O',
        'Ù'..='Ü' | 'ù'..='ü' => 'U',
        'Ý' | 'ý' | 'ÿ' => 'Y',
        _ => return None,
    })
}

/// Tests the start of a registered name without skipping any words or punctuation.
pub fn name_starts_with(name: &str, ascii_prefix: &str) -> bool {
    let mut name = name.trim_start().chars();
    ascii_prefix.bytes().all(|expected| name.next().and_then(fold) == Some(char::from(expected.to_ascii_uppercase())))
}

/// Words that carry no search value in company names and would create huge postings.
pub const STOPWORDS: &[&str] = &["DA", "DAS", "DE", "DI", "DO", "DOS", "E"];

/// Longest indexed token, in bytes. Longer tokens are truncated (derived data only).
pub const MAX_TOKEN: usize = 32;

/// Splits text into normalized search tokens, skipping stopwords, preserving order and
/// duplicates.
pub fn tokenize(text: &str, mut f: impl FnMut(&str)) {
    let mut tok = String::with_capacity(32);
    let mut flush = |tok: &mut String| {
        if !tok.is_empty() {
            if !STOPWORDS.contains(&tok.as_str()) {
                f(tok);
            }
            tok.clear();
        }
    };
    for c in text.chars() {
        match fold(c) {
            Some(u) => {
                if tok.len() < MAX_TOKEN {
                    tok.push(u);
                }
            }
            None => flush(&mut tok),
        }
    }
    flush(&mut tok);
}

/// Whether a normalized token goes into the search index. 11-digit numbers are excluded:
/// in company names they are almost always the owner's CPF (individual entrepreneurs and
/// MEI), so indexing them would allow looking people up by CPF (LGPD data minimization)
/// and would cost ~17 million unique terms. The values themselves are stored unchanged.
pub fn is_indexed_token(t: &str) -> bool {
    !(t.len() == 11 && t.bytes().all(|b| b.is_ascii_digit()))
}

pub fn tokens(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    tokenize(text, |t| v.push(t.to_string()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin1() {
        let mut s = String::new();
        latin1_to_string(b"S\xc3O PAULO \xe7", &mut s);
        assert_eq!(s, "SÃO PAULO ç");
    }

    #[test]
    fn name_prefix_matches_only_the_start_of_the_registered_name() {
        assert!(name_starts_with("  ZAF-LAS LTDA", "za"));
        assert!(name_starts_with("ÓTICA", "OT"));
        assert!(!name_starts_with("CASA ZAF-LAS", "ZA"));
        assert!(!name_starts_with("DE ZAF-LAS", "ZA"));
        assert!(!name_starts_with("-ZAF-LAS", "ZA"));
        assert!(!name_starts_with("", "Z"));
    }

    #[test]
    fn tokenization() {
        assert_eq!(tokens("Padaria São João & Cia. Ltda - ME"), ["PADARIA", "SAO", "JOAO", "CIA", "LTDA", "ME"]);
        assert_eq!(tokens("BANCO DO BRASIL SA"), ["BANCO", "BRASIL", "SA"]);
        assert_eq!(tokens("  "), Vec::<String>::new());
        assert_eq!(tokens("WM&R"), ["WM", "R"]);
        assert_eq!(tokens(&"X".repeat(50))[0].len(), MAX_TOKEN);
        assert!(!is_indexed_token("12345678901"));
        assert!(is_indexed_token("1234567890") && is_indexed_token("ABCDEFGHIJK"));
    }
}
