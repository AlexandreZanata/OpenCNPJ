//! Inclusive date ranges anchored to the served data, not the wall clock.

use opencnpj_query::ir::*;

pub fn has(text: &str, phrases: &[&str]) -> bool {
    let padded = format!(" {text} ");
    phrases.iter().any(|p| padded.contains(&format!(" {p} ")))
}

pub fn number(token: &str) -> Option<u64> {
    let small = match token {
        "zero" => Some(0),
        "um" | "uma" | "primeiro" | "primeira" => Some(1),
        "dois" | "duas" | "segundo" | "segunda" => Some(2),
        "tres" | "terceiro" | "terceira" => Some(3),
        "quatro" => Some(4),
        "cinco" => Some(5),
        "seis" => Some(6),
        "sete" => Some(7),
        "oito" => Some(8),
        "nove" => Some(9),
        "dez" => Some(10),
        "vinte" => Some(20),
        "trinta" => Some(30),
        "cinquenta" => Some(50),
        "sessenta" => Some(60),
        "setenta" => Some(70),
        "oitenta" => Some(80),
        "noventa" => Some(90),
        "cem" => Some(100),
        _ => None,
    };
    small.or_else(|| token.trim_end_matches(['o', 'a', 'º', 'ª']).replace('.', "").parse().ok())
}

pub fn quantity(tokens: &[String], at: usize) -> Option<(u64, usize)> {
    let token = tokens.get(at)?;
    if let Some(k) = token.strip_suffix('k') {
        return number(k)?.checked_mul(1000).map(|n| (n, at + 1));
    }
    let mut n = number(token)?;
    let mut end = at + 1;
    if let Some(scale) = tokens.get(end).and_then(|s| match s.as_str() {
        "mil" => Some(1000),
        "milhao" | "milhoes" => Some(1_000_000),
        _ => None,
    }) {
        n = n.checked_mul(scale)?;
        end += 1;
    }
    Some((n, end))
}

pub fn years(text: &str) -> Vec<u16> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|s| s.len() == 4)
        .filter_map(|s| s.parse::<u16>().ok())
        .filter(|y| (1900..=2100).contains(y))
        .collect()
}

pub fn competences(text: &str) -> Vec<Competence> {
    text.split_whitespace()
        .filter_map(|s| {
            let (y, m) = s.trim_matches(|c: char| !c.is_ascii_digit()).split_once('-')?;
            Competence::from_year_month(y.parse().ok()?, m.parse().ok()?)
        })
        .collect()
}

pub(crate) fn date_shaped(word: &str) -> bool {
    let parts: Vec<_> = word.trim_matches([',', '.']).split(['/', '-']).collect();
    parts.len() == 3
        && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && ((parts[0].len() == 4 && parts[1].len() <= 2 && parts[2].len() <= 2)
            || (parts[0].len() <= 2 && parts[1].len() <= 2 && parts[2].len() == 4))
}

fn literal_date(word: &str) -> Option<Day> {
    let parts: Vec<_> = word.trim_matches([',', '.']).split(['/', '-']).collect();
    let a = parts.first()?.parse::<u16>().ok()?;
    let b = parts.get(1)?.parse::<u8>().ok()?;
    let c = parts.get(2)?.parse::<u16>().ok()?;
    if parts[0].len() == 4 { Day::from_ymd(a, b, u8::try_from(c).ok()?) } else { Day::from_ymd(c, b, u8::try_from(a).ok()?) }
}

pub(crate) fn invalid_dates(text: &str) -> bool {
    text.split_whitespace().any(|word| date_shaped(word) && literal_date(word).is_none())
}

pub fn period(text: &str, anchor: Day) -> Option<Period> {
    let (year, month, anchor_day) = anchor.to_ymd()?;
    let ys = years(text);
    let words: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    if let Some(at) = words.iter().position(|s| s == "ultimos" || s == "ultimas") {
        let (n, end) = quantity(&words, at + 1)?;
        if n == 0 {
            return None;
        }
        let from = match words.get(end)?.as_str() {
            "dias" => Day(anchor.0.checked_sub(u32::try_from(n).ok()?.checked_sub(1)?)?),
            "meses" | "anos" => {
                let months = if words[end] == "anos" { n.checked_mul(12)? } else { n };
                let absolute = u64::from(year).checked_mul(12)?.checked_add(u64::from(month - 1))?.checked_sub(months)?;
                let y = u16::try_from(absolute / 12).ok()?;
                let m = (absolute % 12) as u8 + 1;
                let last = month_end(y, m)?.to_ymd()?.2;
                Day(Day::from_ymd(y, m, anchor_day.min(last))?.0.checked_add(1)?)
            }
            _ => return None,
        };
        return Some(Period::Days(Range { from: Some(from), to: Some(anchor) }));
    }
    let explicit: Vec<Day> = words.iter().filter(|word| date_shaped(word)).map(|word| literal_date(word)).collect::<Option<Vec<_>>>()?;
    if let Some(&from) = explicit.first() {
        let to = explicit.get(1).copied().unwrap_or(if has(text, &["desde", "a partir"]) { anchor } else { from });
        return Some(Period::Days(Range { from: Some(from), to: Some(to) }));
    }
    let named_month =
        ["janeiro", "fevereiro", "marco", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"]
            .iter()
            .any(|m| has(text, &[m]));
    if has(text, &["recentemente"]) {
        return Some(Period::Days(Range { from: Some(Day(anchor.0.saturating_sub(89))), to: Some(anchor) }));
    }
    let target = if has(text, &["ano passado"]) {
        year.checked_sub(1)?
    } else if has(text, &["este ano", "neste ano", "esse ano"]) {
        year
    } else {
        ys.first().copied().or(if named_month { Some(year) } else { None })?
    };
    if target > year {
        return Some(Period::Days(Range { from: Some(Day::from_ymd(target, 1, 1)?), to: Some(Day::from_ymd(target, 12, 31)?) }));
    }
    if let Some(c) = competences(text).first() {
        return Some(Period::Days(Range {
            from: Day::from_ymd(c.year(), c.month(), 1),
            to: Some(month_end(c.year(), c.month())?.min(anchor)),
        }));
    }
    let mut from = Day::from_ymd(target, 1, 1)?;
    let endyear = ys.last().copied().unwrap_or(target);
    let mut to = if endyear >= year { anchor } else { Day::from_ymd(endyear, 12, 31)? };
    for (i, name) in
        ["janeiro", "fevereiro", "marco", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"]
            .iter()
            .enumerate()
    {
        if has(text, &[name]) {
            let m = i as u8 + 1;
            let y = if ys.is_empty() && m > month { year - 1 } else { target };
            from = Day::from_ymd(y, m, 1)?;
            if !has(text, &["desde", "a partir"]) {
                to = month_end(y, m)?.min(anchor);
            }
            break;
        }
    }
    if has(text, &["primeiro semestre", "1 semestre"]) {
        from = Day::from_ymd(target, 1, 1)?;
        to = Day::from_ymd(target, 6, 30)?.min(anchor);
    }
    if has(text, &["segundo semestre", "2 semestre"]) {
        from = Day::from_ymd(target, 7, 1)?;
        to = Day::from_ymd(target, 12, 31)?.min(anchor);
    }
    for q in 1..=4 {
        if has(text, &[&format!("{q} trimestre"), &format!("{q} tri")]) {
            from = Day::from_ymd(target, (q - 1) * 3 + 1, 1)?;
            to = month_end(target, q * 3)?.min(anchor);
        }
    }
    Some(Period::Days(Range { from: Some(from), to: Some(to) }))
}

fn month_end(y: u16, m: u8) -> Option<Day> {
    let next = if m == 12 { Day::from_ymd(y + 1, 1, 1)? } else { Day::from_ymd(y, m + 1, 1)? };
    Some(Day(next.0 - 1))
}

/// Brazilian currency in exact integer cents, including decimal comma and scales.
pub fn money_quantity(tokens: &[String], at: usize) -> Option<(u64, usize)> {
    let token = tokens.get(at)?;
    if let Some((whole, fraction)) = token.split_once(',') {
        if fraction.is_empty() || fraction.len() > 2 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let whole = whole.replace('.', "").parse::<u64>().ok()?;
        let fraction = fraction.parse::<u64>().ok()?.checked_mul(if fraction.len() == 1 { 10 } else { 1 })?;
        let mut cents = whole.checked_mul(100)?.checked_add(fraction)?;
        let mut end = at + 1;
        if let Some(scale) = tokens.get(end).and_then(|s| match s.as_str() {
            "mil" => Some(1000),
            "milhao" | "milhoes" => Some(1_000_000),
            _ => None,
        }) {
            cents = cents.checked_mul(scale)?;
            end += 1;
        }
        return Some((cents, end));
    }
    quantity(tokens, at).and_then(|(value, end)| value.checked_mul(100).map(|value| (value, end)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchor_and_quantities() {
        let a = Day::from_ymd(2026, 10, 11).unwrap();
        assert_eq!(
            period("ano passado", a),
            Some(Period::Days(Range { from: Day::from_ymd(2025, 1, 1), to: Day::from_ymd(2025, 12, 31) }))
        );
        assert_eq!(quantity(&["50".into(), "mil".into()], 0), Some((50000, 2)));
        assert_eq!(number("microempresa"), None);
    }
}
