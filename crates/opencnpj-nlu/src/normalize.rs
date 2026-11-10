//! Bounded Unicode normalization and token-level colloquial rewrites.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Normalize accents and punctuation without changing substrings inside entity names.
pub fn fold(input: &str) -> String {
    input
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | ',' | '/' | '-' | '%') { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn tokens(input: &str) -> Vec<String> {
    fold(input)
        .split_whitespace()
        .map(|word| {
            match word.trim_matches(['.', ',', ';']) {
                "qtas" | "qntas" | "qts" => "quantas",
                "qtos" | "qnts" | "qntos" => "quantos",
                "hj" => "hoje",
                "pra" => "para",
                "pro" => "para",
                "vc" => "voce",
                "tb" | "tbm" => "tambem",
                "estabelcimentos" => "estabelecimentos",
                "meis" => "mei",
                "padoca" => "padaria",
                "padocas" => "padarias",
                "farmacis" => "farmacias",
                "hotels" => "hoteis",
                "cnpjs" => "cnpjs",
                other => other,
            }
            .to_owned()
        })
        .collect()
}

/// Entity keys ignore separators; numeric parsers retain the original tokens.
pub fn words(input: &str) -> Vec<String> {
    tokens(input)
        .into_iter()
        .flat_map(|s| s.split(['.', ',', '/', '-', '%']).filter(|p| !p.is_empty()).map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_rewrites_preserve_names() {
        assert_eq!(super::tokens("Qtas farmácias em Cuiabá?"), ["quantas", "farmacias", "em", "cuiaba"]);
        assert_eq!(super::tokens("Prado Contagem"), ["prado", "contagem"]);
    }
}
