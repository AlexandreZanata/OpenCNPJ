//! Delivery skills layered over the data question.
//!
//! A delivery phrase ("me mande um Excel com…", "bah, tchê, me passa uma planilha", "em
//! CSV", "apenas o nome e o CNPJ") is cut from the question before the grammar runs, so
//! its words never act as a place, sector, filter or intent. The rest is interpreted like
//! any other question, and the removed span becomes a typed [`Export`]. The vocabulary is
//! data (`lang/pt-BR/delivery.toml`), whose lists include each other recursively.

use std::collections::{BTreeMap, BTreeSet};

use crate::normalize;
use opencnpj_query::export::{Export, ExportField, ExportFormat};

pub(crate) struct Delivery {
    pub export: Export,
    /// The question without the delivery phrase, in its original spelling and case.
    pub rest: String,
    pub notes: Vec<String>,
    /// A format the service cannot write (PDF, Word…) was asked for.
    pub unsupported: Option<String>,
}

type Phrase = Vec<String>;

/// Phrase lists of the delivery vocabulary, each sorted longest first.
#[derive(Debug)]
pub(crate) struct Vocabulary {
    formats: Vec<(Phrase, ExportFormat)>,
    unsupported: Vec<Phrase>,
    request: Vec<Phrase>,
    record_openers: Vec<Phrase>,
    record_commands: Vec<Phrase>,
    actions: Vec<Phrase>,
    export_verbs: Vec<Phrase>,
    file_nouns: Vec<Phrase>,
    left_glue: Vec<Phrase>,
    right_glue: Vec<Phrase>,
    intros: Vec<Phrase>,
    articles: Vec<Phrase>,
    previous: Vec<Phrase>,
    tail: Vec<Phrase>,
    company: Vec<Phrase>,
    company_nouns: Vec<Phrase>,
    /// Column phrases, the columns they name, and whether the phrase is a bare "número".
    fields: Vec<(Phrase, Vec<ExportField>, bool)>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    lists: BTreeMap<String, Vec<String>>,
    formats: BTreeMap<String, Vec<String>>,
    roles: BTreeMap<String, Vec<String>>,
    fields: BTreeMap<String, Vec<String>>,
}

/// Expand `items`, replacing "@name" by the list `name`, recursively. `path` holds the
/// lists being expanded, so a list that includes itself is reported instead of looping.
fn expand(
    items: &[String],
    lists: &BTreeMap<String, Vec<String>>,
    path: &mut Vec<String>,
    out: &mut BTreeSet<Phrase>,
) -> Result<(), String> {
    for item in items {
        if let Some(name) = item.strip_prefix('@') {
            if path.iter().any(|seen| seen == name) {
                return Err(format!("delivery vocabulary cycle: {} -> {name}", path.join(" -> ")));
            }
            let list = lists.get(name).ok_or_else(|| format!("unknown delivery list @{name}"))?;
            path.push(name.to_owned());
            expand(list, lists, path, out)?;
            path.pop();
        } else {
            let phrase = normalize::words(item);
            if phrase.is_empty() {
                return Err(format!("empty delivery phrase {item:?}"));
            }
            out.insert(phrase);
        }
    }
    Ok(())
}

fn longest_first(phrases: BTreeSet<Phrase>) -> Vec<Phrase> {
    let mut phrases: Vec<Phrase> = phrases.into_iter().collect();
    phrases.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    phrases
}

impl Vocabulary {
    pub(crate) fn embedded() -> Self {
        Self::parse(include_str!("../lang/pt-BR/delivery.toml")).expect("embedded delivery vocabulary")
    }

    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let resolve = |items: &[String]| -> Result<Vec<Phrase>, String> {
            let mut out = BTreeSet::new();
            expand(items, &file.lists, &mut Vec::new(), &mut out)?;
            Ok(longest_first(out))
        };
        let section = |map: &BTreeMap<String, Vec<String>>, key: &str| -> Result<Vec<Phrase>, String> {
            resolve(map.get(key).ok_or_else(|| format!("missing delivery entry {key}"))?)
        };
        // Every list must resolve, including the ones no role uses yet.
        for list in file.lists.values() {
            resolve(list)?;
        }
        let mut formats = Vec::new();
        for (name, format) in [("xlsx", ExportFormat::Xlsx), ("csv", ExportFormat::Csv), ("txt", ExportFormat::Txt)] {
            formats.extend(section(&file.formats, name)?.into_iter().map(|phrase| (phrase, format)));
        }
        formats.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        let mut fields = Vec::new();
        for (name, items) in &file.fields {
            let (columns, bare) = match name.as_str() {
                "all" => (ExportField::ALL.to_vec(), false),
                "cnpj_number" => (vec![ExportField::Cnpj], true),
                "contacts" => (vec![ExportField::Telefone, ExportField::Email], false),
                "full_address" => (vec![ExportField::Endereco, ExportField::Bairro, ExportField::Cep], false),
                other => {
                    let field: ExportField = serde_json::from_value(serde_json::Value::String(other.to_owned()))
                        .map_err(|_| format!("unknown export field {other}"))?;
                    (vec![field], false)
                }
            };
            fields.extend(resolve(items)?.into_iter().map(|phrase| (phrase, columns.clone(), bare)));
        }
        fields.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Ok(Self {
            formats,
            unsupported: section(&file.formats, "unsupported")?,
            request: section(&file.roles, "request")?,
            record_openers: section(&file.roles, "record_openers")?,
            record_commands: section(&file.roles, "record_commands")?,
            actions: section(&file.roles, "actions")?,
            export_verbs: section(&file.roles, "export_verbs")?,
            file_nouns: section(&file.roles, "file_nouns")?,
            left_glue: section(&file.roles, "left_glue")?,
            right_glue: section(&file.roles, "right_glue")?,
            intros: section(&file.roles, "intros")?,
            articles: section(&file.roles, "articles")?,
            previous: section(&file.roles, "previous")?,
            tail: section(&file.roles, "tail")?,
            company: section(&file.roles, "company")?,
            company_nouns: section(&file.roles, "company_nouns")?,
            fields,
        })
    }
}

struct Word {
    key: String,
    token: usize,
    /// The original token ends with a comma after this word ("nome, CNPJ").
    comma: bool,
}

fn words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    for (token, raw) in text.split_whitespace().enumerate() {
        let parts = normalize::words(raw);
        let comma = raw.trim_end_matches(['.', '?', '!', ';', ':', ')']).ends_with(',');
        let last = parts.len().saturating_sub(1);
        out.extend(parts.into_iter().enumerate().map(|(i, key)| Word { key, token, comma: comma && i == last }));
    }
    out
}

fn matches_at(words: &[Word], at: usize, phrase: &[String]) -> bool {
    at + phrase.len() <= words.len() && words[at..at + phrase.len()].iter().zip(phrase).all(|(w, p)| w.key == *p)
}

/// End of the longest phrase of `list` starting at `at`.
fn at(list: &[Phrase], words: &[Word], at: usize) -> Option<usize> {
    list.iter().find(|phrase| matches_at(words, at, phrase)).map(|phrase| at + phrase.len())
}

/// Start of the longest phrase of `list` ending at `end`.
fn ending(list: &[Phrase], words: &[Word], end: usize) -> Option<usize> {
    list.iter()
        .filter(|phrase| phrase.len() <= end)
        .find(|phrase| matches_at(words, end - phrase.len(), phrase))
        .map(|phrase| end - phrase.len())
}

struct FieldList {
    end: usize,
    fields: Vec<ExportField>,
    named: usize,
    bare_number: bool,
}

impl Vocabulary {
    /// Words before a record request, stopping at the first catalogue entity.
    pub(crate) fn record_opening(&self, words: &[String]) -> (usize, bool) {
        let mut end = 0;
        let mut commands = false;
        while end < words.len() {
            let rest = &words[end..];
            let opener = self.record_openers.iter().find(|phrase| rest.starts_with(phrase)).map_or(0, Vec::len);
            let command = self.record_commands.iter().find(|phrase| rest.starts_with(phrase)).map_or(0, Vec::len);
            if opener == 0 && command == 0 {
                break;
            }
            commands |= command >= opener;
            end += opener.max(command);
        }
        (end, commands)
    }

    /// Ignore only a complete courtesy tag at the end, outside catalogue entities.
    pub(crate) fn record_closing(&self, words: &[String], mut end: usize) -> usize {
        while let Some(phrase) = self.tail.iter().find(|phrase| words[..end].ends_with(phrase)) {
            end -= phrase.len();
        }
        end
    }

    fn field_at(&self, words: &[Word], start: usize) -> Option<(usize, &[ExportField], bool)> {
        self.fields
            .iter()
            .find(|(phrase, _, _)| matches_at(words, start, phrase))
            .map(|(phrase, fields, bare)| (start + phrase.len(), fields.as_slice(), *bare))
    }

    /// `[article] field ((, | e | , e) [article] field)*`, then one optional genitive.
    fn field_list(&self, words: &[Word], mut next: usize) -> Option<FieldList> {
        let mut list = FieldList { end: next, fields: Vec::new(), named: 0, bare_number: false };
        loop {
            let start = at(&self.articles, words, next).unwrap_or(next);
            let Some((end, fields, bare)) = self.field_at(words, start) else { break };
            for field in fields {
                if !list.fields.contains(field) {
                    list.fields.push(*field);
                }
            }
            list.named += 1;
            list.bare_number |= bare;
            list.end = end;
            next = end;
            let comma = words[end - 1].comma;
            if words.get(next).is_some_and(|w| matches!(w.key.as_str(), "e" | "mais")) {
                next += 1;
            } else if !comma {
                break;
            }
        }
        if list.named == 0 {
            return None;
        }
        // A list ends at the end of the question, at punctuation or before a connector.
        let boundary = list.end == words.len()
            || words[list.end - 1].comma
            || matches!(
                words[list.end].key.as_str(),
                "de" | "do" | "da" | "dos" | "das" | "em" | "no" | "na" | "para" | "num" | "numa" | "que"
            )
            || self.format_at(words, list.end).is_some()
            || at(&self.request, words, list.end).is_some();
        if !boundary {
            return None;
        }
        if words.get(list.end).is_some_and(|w| matches!(w.key.as_str(), "de" | "do" | "da" | "dos" | "das")) {
            list.end += 1;
        }
        Some(list)
    }

    fn format_at(&self, words: &[Word], start: usize) -> Option<(usize, ExportFormat)> {
        self.formats.iter().find(|(phrase, _)| matches_at(words, start, phrase)).map(|(phrase, format)| (start + phrase.len(), *format))
    }

    /// A file noun ("uma tabela", "um relatório") preceded by a hand-over or make verb.
    fn requested_file(&self, words: &[Word]) -> Option<(usize, usize)> {
        (0..words.len()).find_map(|start| {
            let end = at(&self.file_nouns, words, start)?;
            let mut left = start;
            while let Some(from) = ending(&self.left_glue, words, left).filter(|from| *from < left) {
                left = from;
            }
            ending(&self.actions, words, left).map(|_| (start, end))
        })
    }

    /// Extract a file delivery. `previous` is the export of the conversation, which a bare
    /// column list ("só o nome e o CNPJ") refines.
    pub(crate) fn delivery(&self, text: &str, previous: Option<&Export>) -> Option<Delivery> {
        let words = words(text);
        let format = (0..words.len()).find_map(|start| {
            let (mut end, format) = self.format_at(&words, start)?;
            // A comma can introduce another name for the same format ("um CSV, em formato CSV").
            while words[end - 1].comma {
                let mut next = end;
                while let Some(after_glue) = at(&self.left_glue, &words, next) {
                    next = after_glue;
                }
                let Some((after_format, _)) = self.format_at(&words, next).filter(|(_, other)| *other == format) else { break };
                end = after_format;
            }
            Some((start, end, format))
        });
        let unsupported = (0..words.len()).find_map(|start| at(&self.unsupported, &words, start).map(|end| (start, end)));
        let noun = if format.is_none() { self.requested_file(&words) } else { None };
        let verb = (0..words.len()).find_map(|start| at(&self.export_verbs, &words, start).map(|end| (start, end)));
        let mut removed = vec![false; words.len()];
        let mut notes = Vec::new();
        let mut fields: Vec<ExportField> = Vec::new();
        let mut bare_number = false;
        let mut take = |list: &FieldList, start: usize, removed: &mut Vec<bool>| {
            for field in &list.fields {
                if !fields.contains(field) {
                    fields.push(*field);
                }
            }
            bare_number |= list.bare_number;
            removed[start..list.end].iter_mut().for_each(|r| *r = true);
        };
        let unsupported = unsupported.filter(|_| format.is_none());
        let delivered = format.is_some() || verb.is_some() || noun.is_some() || unsupported.is_some();
        if !delivered && previous.is_none() {
            return None;
        }
        // Column lists: after an explicit introduction, or right after a request word.
        let mut start = 0;
        while start < words.len() {
            let intro = at(&self.intros, &words, start);
            let after_request = if delivered { at(&self.request, &words, start) } else { None };
            let opened = intro.or(after_request);
            if let Some(end) = opened
                && let Some(list) = self.field_list(&words, end)
            {
                take(&list, if intro.is_some() { start } else { end }, &mut removed);
                start = list.end;
                continue;
            }
            start += 1;
        }
        // The file itself: a format, an unsupported format or a requested file noun.
        let core = format.map(|(s, e, _)| (s, e)).or(unsupported).or(noun);
        if let Some((start, end)) = core {
            removed[start..end].iter_mut().for_each(|r| *r = true);
            // Tags after the file ("um CSV aí", "uma planilha, por favor") belong to it.
            let mut end = end;
            while let Some(tag_end) = at(&self.tail, &words, end) {
                removed[end..tag_end].iter_mut().for_each(|r| *r = true);
                end = tag_end;
            }
            // "com"/"contendo" right after the file introduces columns even for one name;
            // so does a list placed right after it ("numa planilha o nome e o CEP das …").
            let genitive = |list: &FieldList| matches!(words[list.end - 1].key.as_str(), "de" | "do" | "da" | "dos" | "das");
            if let Some(list) = self.field_list(&words, end).filter(genitive) {
                take(&list, end, &mut removed);
            } else if let Some(glue_end) = at(&self.right_glue, &words, end) {
                let opener = matches!(words[end].key.as_str(), "com" | "contendo");
                match self.field_list(&words, glue_end).filter(|_| opener) {
                    Some(list) => take(&list, end, &mut removed),
                    None => removed[end..glue_end].iter_mut().for_each(|r| *r = true),
                }
            }
            self.cut_left(&words, start, &mut removed);
        }
        if let Some((start, end)) = verb {
            // "Exporte as farmácias…": the verb and the request words before it.
            removed[start..end].iter_mut().for_each(|r| *r = true);
            self.cut_left(&words, start, &mut removed);
            let mut right = end;
            while core.is_none() && words.get(right).is_some_and(|w| matches!(w.key.as_str(), "para" | "em" | "como")) {
                removed[right] = true;
                right += 1;
            }
        }
        if !delivered && fields.is_empty() {
            return None;
        }
        if delivered {
            // A request opening the question ("Bah, tchê, me passa …") is cut as well.
            let mut start = 0;
            while start < words.len() {
                if removed[start] || matches!(words[start].key.as_str(), "agora") {
                    start += 1;
                } else if let Some(end) = at(&self.request, &words, start) {
                    removed[start..end].iter_mut().for_each(|r| *r = true);
                    start = end;
                } else {
                    break;
                }
            }
            // So are courtesy and regional tags closing it ("…, visse?", "valeu").
            let mut end = words.len();
            while end > 0 {
                if removed[end - 1] {
                    end -= 1;
                } else if let Some(from) = ending(&self.tail, &words, end) {
                    removed[from..end].iter_mut().for_each(|r| *r = true);
                    end = from;
                } else {
                    break;
                }
            }
        }
        // Keep a token unless every word in it belongs to the delivery phrase.
        let rest = text
            .split_whitespace()
            .enumerate()
            .filter(|(token, _)| {
                words.iter().zip(&removed).any(|(w, r)| w.token == *token && !*r) || words.iter().all(|w| w.token != *token)
            })
            .map(|(_, raw)| raw)
            .collect::<Vec<_>>()
            .join(" ");
        let format = match (format, previous) {
            (Some((_, _, format)), _) => format,
            (None, Some(previous)) if !delivered => previous.format,
            _ => {
                if unsupported.is_none() {
                    notes.push("Gerei o arquivo em Excel (.xlsx). Peça CSV ou TXT se preferir outro formato.".into());
                }
                ExportFormat::Xlsx
            }
        };
        if bare_number {
            notes.push("Considerei “número” como o número de inscrição no CNPJ. Para contatos, peça o telefone.".into());
        }
        // "Agora em CSV" changes the format of the previous file and keeps its columns.
        let rest = rest.trim_matches([',', ';', ' ']).to_owned();
        let fields = match previous {
            Some(previous) if fields.is_empty() && (!delivered || self.refers_to_previous(&rest)) => previous.fields.clone(),
            _ => fields,
        };
        let unsupported =
            unsupported.map(|(start, end)| words[start..end].iter().map(|w| w.key.to_uppercase()).collect::<Vec<_>>().join(" "));
        Some(Delivery { export: Export { format, fields, rows: None }, rest, notes, unsupported })
    }

    /// Cut the glue and request words that lead to a file phrase starting at `start`.
    fn cut_left(&self, words: &[Word], start: usize, removed: &mut [bool]) {
        let mut left = start;
        while left > 0 {
            if removed[left - 1] {
                left -= 1;
                continue;
            }
            let from = ending(&self.left_glue, words, left)
                .or_else(|| ending(&self.file_nouns, words, left))
                .or_else(|| ending(&self.request, words, left));
            let Some(from) = from else { break };
            removed[from..left].iter_mut().for_each(|r| *r = true);
            left = from;
        }
    }

    /// The company named after a search phrase ("encontre a empresa Zaflas", "qual o CNPJ
    /// da Padaria Pão Quente?"), in its original spelling. With `bare`, a short question
    /// that is only a name ("zaflas") counts too.
    pub(crate) fn company_name(&self, text: &str, bare: bool) -> Option<String> {
        let words = words(text);
        let mut start = 0;
        let mut opened = false;
        while start < words.len() {
            if let Some(end) = at(&self.company, &words, start) {
                start = end;
                opened = true;
                break;
            }
            match at(&self.request, &words, start) {
                Some(end) => start = end,
                None => break,
            }
        }
        if !opened && (start > 0 || !bare) {
            return None;
        }
        while let Some(end) = at(&self.company_nouns, &words, start).or_else(|| at(&self.articles, &words, start)) {
            start = end;
        }
        let mut end = words.len();
        while let Some(from) = ending(&self.tail, &words, end).filter(|from| *from > start) {
            end = from;
        }
        if start >= end {
            return None;
        }
        if !opened {
            // A bare name: a few words, no question, not conversational, not a dialogue connective,
            // not only courtesy or greetings.
            let question = words[start..end].iter().any(|w| {
                matches!(
                    w.key.as_str(),
                    "quantos"
                        | "quantas"
                        | "quanto"
                        | "quanta"
                        | "qual"
                        | "quais"
                        | "como"
                        | "onde"
                        | "quando"
                        | "porque"
                        | "por"
                        | "que"
                        | "quem"
                )
            });
            let connective = matches!(
                words[start].key.as_str(),
                "e" | "so" | "apenas" | "tambem" | "ou" | "mas" | "ja" | "porte" | "do" | "da" | "de" | "no" | "na" | "em"
            ) || (end - start >= 2 && words[start].key == "na" && words[start + 1].key == "verdade")
                || (end - start >= 2 && matches!(words[start].key.as_str(), "a" | "o") && words[start + 1].key == "do");
            let conversational = words[start..end]
                .iter()
                .any(|w| matches!(w.key.as_str(), "vai" | "chover" | "chuva" | "clima" | "previsao" | "amanha" | "ontem" | "hoje"));
            let filler =
                (start..end).all(|i| [&self.request, &self.tail, &self.previous].iter().any(|list| ending(list, &words, i + 1).is_some()));
            // A trailing place ("… em Sorriso/MT") is checked by the caller, not counted here.
            let named = words[start..end].iter().rposition(|w| matches!(w.key.as_str(), "em" | "no" | "na")).unwrap_or(end - start);
            if named > 5 || question || connective || conversational || filler {
                return None;
            }
        }
        let (first, last) = (words[start].token, words[end - 1].token);
        let name = text.split_whitespace().skip(first).take(last - first + 1).collect::<Vec<_>>().join(" ");
        let name = name.trim_matches(|c: char| c.is_ascii_punctuation() || matches!(c, '“' | '”' | '«' | '»')).trim().to_owned();
        (!name.is_empty()).then_some(name)
    }

    /// A delivery follow-up whose remaining words only point at the previous answer.
    pub(crate) fn refers_to_previous(&self, rest: &str) -> bool {
        let words = words(rest);
        let mut start = 0;
        while start < words.len() {
            let end = [&self.previous, &self.request, &self.tail].iter().filter_map(|list| at(list, &words, start)).max();
            match end {
                Some(end) => start = end,
                None => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> Vocabulary {
        Vocabulary::embedded()
    }

    fn cut(text: &str) -> (Export, String) {
        let d = vocabulary().delivery(text, None).unwrap_or_else(|| panic!("no delivery in {text}"));
        (d.export, d.rest)
    }

    #[test]
    fn lists_include_each_other_and_cycles_fail() {
        let text = |lists: &str| {
            format!(
                "[lists]\n{lists}\n[formats]\nxlsx=[\"excel\"]\ncsv=[\"csv\"]\ntxt=[\"txt\"]\nunsupported=[\"pdf\"]\n[roles]\nrequest=[\"@a\"]\nrecord_openers=[\"@a\"]\nrecord_commands=[\"manda\"]\nactions=[\"manda\"]\nexport_verbs=[\"exporte\"]\nfile_nouns=[\"arquivo\"]\nleft_glue=[\"um\"]\nright_glue=[\"com\"]\nintros=[\"apenas\"]\narticles=[\"o\"]\nprevious=[\"isso\"]\ntail=[\"valeu\"]\ncompany=[\"encontre\"]\ncompany_nouns=[\"a empresa\"]\n[fields]\ncnpj=[\"cnpj\"]\n"
            )
        };
        let nested = Vocabulary::parse(&text("a=[\"@b\", \"Por Favor\"]\nb=[\"@c\", \"me\"]\nc=[\"Tchê\"]")).unwrap();
        assert_eq!(nested.request, [vec!["por".to_owned(), "favor".to_owned()], vec!["me".to_owned()], vec!["tche".to_owned()]]);
        assert!(Vocabulary::parse(&text("a=[\"@b\"]\nb=[\"@a\"]")).unwrap_err().contains("cycle"));
        assert!(Vocabulary::parse(&text("a=[\"@missing\"]")).unwrap_err().contains("unknown"));
    }

    #[test]
    fn format_and_request_words_are_cut() {
        let (export, rest) = cut("me mande um exel com todas as empresas que tiveram baixa em mato grosso no setor de TI");
        assert_eq!(export.format, ExportFormat::Xlsx);
        assert!(export.fields.is_empty());
        assert_eq!(rest, "todas as empresas que tiveram baixa em mato grosso no setor de TI");
        let (export, rest) = cut("Liste as padarias de Cuiabá/MT em CSV.");
        assert_eq!(export.format, ExportFormat::Csv);
        assert_eq!(rest, "Liste as padarias de Cuiabá/MT");
        let (export, rest) = cut("Exporte as farmácias ativas de Sinop MT");
        assert_eq!(export.format, ExportFormat::Xlsx);
        assert_eq!(rest, "as farmácias ativas de Sinop MT");
    }

    #[test]
    fn column_lists_are_typed() {
        let (export, rest) =
            cut("me mande apenas o nome e o numero em um exel com todas as empresas que tiveram baixa em mato grosso no setor de TI");
        assert_eq!(export.fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
        assert_eq!(rest, "todas as empresas que tiveram baixa em mato grosso no setor de TI");
        let (export, rest) = cut("Gere uma planilha com nome, CNPJ e telefone das padarias de Cuiabá MT");
        assert_eq!(export.fields, [ExportField::RazaoSocial, ExportField::Cnpj, ExportField::Telefone]);
        assert_eq!(rest, "padarias de Cuiabá MT");
        let (export, rest) = cut("Quero um TXT com o CNPJ e a razão social das empresas baixadas em 2025 em Sorriso MT");
        assert_eq!(export.format, ExportFormat::Txt);
        assert_eq!(export.fields, [ExportField::Cnpj, ExportField::RazaoSocial]);
        assert_eq!(rest, "empresas baixadas em 2025 em Sorriso MT");
    }

    #[test]
    fn regional_requests_and_file_nouns_are_cut() {
        let (export, rest) = cut("Bah, tchê, me passa uma planilha das farmácias de Sinop/MT");
        assert_eq!(export.format, ExportFormat::Xlsx);
        assert_eq!(rest, "farmácias de Sinop/MT");
        let (export, rest) = cut("Oxe, gera um CSV aí das padarias de Cuiabá/MT, visse?");
        assert_eq!(export.format, ExportFormat::Csv);
        assert_eq!(rest, "padarias de Cuiabá/MT");
        let (_, rest) = cut("Uai, cê consegue me mandar um ecxel com as academias de Sorriso/MT?");
        assert_eq!(rest, "as academias de Sorriso/MT?");
        let d = vocabulary().delivery("Égua, maninho, me manda uma tabela com nome e telefone das padarias de Sinop/MT", None).unwrap();
        assert_eq!((d.export.format, d.export.fields.clone()), (ExportFormat::Xlsx, vec![ExportField::RazaoSocial, ExportField::Telefone]));
        assert_eq!(d.rest, "padarias de Sinop/MT");
        assert_eq!(d.notes.len(), 1);
        let (export, rest) = cut("Fala, parceiro, bota numa planilha o nome e o endereço completo das padarias de Goiânia/GO");
        assert_eq!(export.fields, [ExportField::RazaoSocial, ExportField::Endereco, ExportField::Bairro, ExportField::Cep]);
        assert_eq!(rest, "padarias de Goiânia/GO");
        let d = vocabulary().delivery("me manda um PDF das padarias de Sinop/MT", None).unwrap();
        assert_eq!(d.unsupported.as_deref(), Some("PDF"));
        // A table shown in the chat is not a file.
        assert!(vocabulary().delivery("me mostra uma tabela das padarias de Sinop/MT por porte", None).is_none());
    }

    #[test]
    fn company_names_follow_search_phrases() {
        let v = vocabulary();
        assert_eq!(v.company_name("encontre a empresa zaflas", false).as_deref(), Some("zaflas"));
        assert_eq!(
            v.company_name("Qual o CNPJ da Padaria Pão Quente em Cuiabá/MT?", false).as_deref(),
            Some("Padaria Pão Quente em Cuiabá/MT")
        );
        assert_eq!(v.company_name("Bah, me fala sobre a firma chamada \"Zaflas\", por favor", false).as_deref(), Some("Zaflas"));
        assert_eq!(v.company_name("dados da empresa MITEL TELEINFORMATICA", false).as_deref(), Some("MITEL TELEINFORMATICA"));
        assert_eq!(v.company_name("quantas padarias existem em Cuiabá", false), None);
        assert_eq!(v.company_name("zaflas", true).as_deref(), Some("zaflas"));
        assert_eq!(v.company_name("bom dia", true), None);
        assert_eq!(v.company_name("quantas lojas tem aqui", true), None);
        assert_eq!(v.company_name("Cerealista Grão Fino em Sorriso/MT", true).as_deref(), Some("Cerealista Grão Fino em Sorriso/MT"));
    }

    #[test]
    fn data_words_are_not_columns() {
        let (export, rest) = cut("Exporte em CSV somente as matrizes ativas de padarias em Sorriso MT");
        assert!(export.fields.is_empty());
        assert_eq!(rest, "somente as matrizes ativas de padarias em Sorriso MT");
        assert!(vocabulary().delivery("quantas padarias ativas existem em Sorriso", None).is_none());
        assert!(vocabulary().delivery("quantas empresas tiveram baixa em 2025 em Sinop MT", None).is_none());
        assert!(vocabulary().delivery("Uai, quantas padarias tem em Cuiabá/MT?", None).is_none());
    }

    #[test]
    fn a_bare_column_list_refines_the_previous_export() {
        let vocabulary = vocabulary();
        let previous = Export { format: ExportFormat::Csv, fields: Vec::new(), rows: None };
        let d = vocabulary.delivery("só o nome e o CNPJ", Some(&previous)).unwrap();
        assert_eq!(d.export.format, ExportFormat::Csv);
        assert_eq!(d.export.fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
        assert!(vocabulary.refers_to_previous(&d.rest));
        let d = vocabulary.delivery("agora em txt", Some(&previous)).unwrap();
        assert_eq!(d.export.format, ExportFormat::Txt);
        assert!(vocabulary.refers_to_previous(&d.rest));
        assert!(vocabulary.refers_to_previous("bah, manda isso de novo"));
    }
}
