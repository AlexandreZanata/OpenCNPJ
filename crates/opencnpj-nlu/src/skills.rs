//! Delivery skills layered over the data question.
//!
//! A delivery phrase ("me mande um Excel com…", "em CSV", "apenas o nome e o CNPJ") is
//! cut from the question before the grammar runs, so its words never act as a place,
//! sector, filter or intent. The rest is interpreted like any other question, and the
//! removed span becomes a typed [`Export`].

use crate::normalize;
use opencnpj_query::export::{Export, ExportField, ExportFormat};

pub(crate) struct Delivery {
    pub export: Export,
    /// The question without the delivery phrase, in its original spelling and case.
    pub rest: String,
    pub notes: Vec<String>,
}

struct Word {
    key: String,
    token: usize,
    /// The original token ends with a comma after this word ("nome, CNPJ").
    comma: bool,
}

const FORMATS: &[(&str, ExportFormat)] = &[
    ("planilha do excel", ExportFormat::Xlsx),
    ("planilha excel", ExportFormat::Xlsx),
    ("planilhas", ExportFormat::Xlsx),
    ("planilha", ExportFormat::Xlsx),
    ("excel", ExportFormat::Xlsx),
    ("exel", ExportFormat::Xlsx),
    ("excell", ExportFormat::Xlsx),
    ("xlsx", ExportFormat::Xlsx),
    ("xls", ExportFormat::Xlsx),
    ("csv", ExportFormat::Csv),
    ("txt", ExportFormat::Txt),
    ("arquivo de texto", ExportFormat::Txt),
    ("arquivo texto", ExportFormat::Txt),
    ("texto simples", ExportFormat::Txt),
    ("texto puro", ExportFormat::Txt),
    ("bloco de notas", ExportFormat::Txt),
    ("formato de texto", ExportFormat::Txt),
    ("formato texto", ExportFormat::Txt),
];

/// Verbs that ask for a file even without naming its format.
const EXPORT_VERBS: &[&str] = &["exporte", "exporta", "exportar", "exportem", "exportacao", "download"];

/// Request and politeness words that may surround a delivery phrase.
const VERBS: &[&str] = &[
    "me",
    "nos",
    "mim",
    "voce",
    "pode",
    "poderia",
    "podes",
    "consegue",
    "conseguiria",
    "por",
    "favor",
    "gostaria",
    "queria",
    "quero",
    "preciso",
    "precisava",
    "mande",
    "manda",
    "mandar",
    "mandem",
    "envie",
    "envia",
    "enviar",
    "enviem",
    "gere",
    "gera",
    "gerar",
    "gerem",
    "crie",
    "cria",
    "criar",
    "monte",
    "monta",
    "montar",
    "prepare",
    "prepara",
    "preparar",
    "faca",
    "faz",
    "fazer",
    "exporte",
    "exporta",
    "exportar",
    "exportem",
    "baixe",
    "baixar",
    "salve",
    "salvar",
    "devolva",
    "devolve",
    "retorne",
    "retorna",
    "extraia",
    "extrair",
    "passe",
    "passa",
    "passar",
    "coloque",
    "coloca",
    "colocar",
    "ola",
    "oi",
    "download",
    "exportacao",
];

/// Articles and prepositions between the request and the format ("em um", "no formato").
const LEFT_GLUE: &[&str] = &[
    "um",
    "uma",
    "o",
    "a",
    "num",
    "numa",
    "em",
    "no",
    "na",
    "para",
    "como",
    "via",
    "de",
    "do",
    "formato",
    "arquivo",
    "arquivos",
    "documento",
    "tipo",
];

/// Words that join the format to the data question ("um Excel com as padarias").
const RIGHT_GLUE: &[&str] = &["com", "contendo", "de", "do", "da", "dos", "das", "sobre", "listando", "mostrando"];

/// Words that open a column list on their own.
const FIELD_INTROS: &[&str] = &[
    "contendo apenas",
    "contendo somente",
    "com as colunas",
    "com os campos",
    "com as informacoes",
    "com apenas",
    "com somente",
    "so com",
    "apenas com",
    "somente com",
    "contendo",
    "apenas",
    "somente",
    "so",
    "unicamente",
    "exclusivamente",
    "colunas",
    "campos",
];

const ARTICLES: &[&str] = &["o", "a", "os", "as", "seu", "sua", "seus", "suas"];
const GENITIVES: &[&str] = &["de", "do", "da", "dos", "das"];

/// Column names, longest phrases first within each spelling family.
const FIELDS: &[(&str, &[ExportField])] = &[
    ("todos os dados disponiveis", &ExportField::ALL),
    ("todos os dados", &ExportField::ALL),
    ("todas as colunas", &ExportField::ALL),
    ("todos os campos", &ExportField::ALL),
    ("todas as informacoes", &ExportField::ALL),
    ("dados completos", &ExportField::ALL),
    ("numero do cnpj", &[ExportField::Cnpj]),
    ("numero de cnpj", &[ExportField::Cnpj]),
    ("numero de inscricao", &[ExportField::Cnpj]),
    ("numero de telefone", &[ExportField::Telefone]),
    ("numero do telefone", &[ExportField::Telefone]),
    ("numeros de telefone", &[ExportField::Telefone]),
    ("numero", &[ExportField::Cnpj]),
    ("numeros", &[ExportField::Cnpj]),
    ("cnpjs", &[ExportField::Cnpj]),
    ("cnpj", &[ExportField::Cnpj]),
    ("inscricao", &[ExportField::Cnpj]),
    ("nome fantasia", &[ExportField::NomeFantasia]),
    ("nomes fantasia", &[ExportField::NomeFantasia]),
    ("fantasia", &[ExportField::NomeFantasia]),
    ("razao social", &[ExportField::RazaoSocial]),
    ("nome empresarial", &[ExportField::RazaoSocial]),
    ("nome da empresa", &[ExportField::RazaoSocial]),
    ("nome das empresas", &[ExportField::RazaoSocial]),
    ("nomes das empresas", &[ExportField::RazaoSocial]),
    ("nome", &[ExportField::RazaoSocial]),
    ("nomes", &[ExportField::RazaoSocial]),
    ("matriz ou filial", &[ExportField::MatrizFilial]),
    ("matriz filial", &[ExportField::MatrizFilial]),
    ("tipo de unidade", &[ExportField::MatrizFilial]),
    ("data da situacao cadastral", &[ExportField::DataSituacao]),
    ("data da situacao", &[ExportField::DataSituacao]),
    ("data da baixa", &[ExportField::DataSituacao]),
    ("data de baixa", &[ExportField::DataSituacao]),
    ("data do encerramento", &[ExportField::DataSituacao]),
    ("data de encerramento", &[ExportField::DataSituacao]),
    ("data do fechamento", &[ExportField::DataSituacao]),
    ("motivo da baixa", &[ExportField::MotivoSituacao]),
    ("motivo da situacao", &[ExportField::MotivoSituacao]),
    ("motivo", &[ExportField::MotivoSituacao]),
    ("situacao cadastral", &[ExportField::Situacao]),
    ("situacao", &[ExportField::Situacao]),
    ("status", &[ExportField::Situacao]),
    ("data de inicio de atividade", &[ExportField::DataInicio]),
    ("data de inicio da atividade", &[ExportField::DataInicio]),
    ("data de inicio", &[ExportField::DataInicio]),
    ("inicio de atividade", &[ExportField::DataInicio]),
    ("inicio da atividade", &[ExportField::DataInicio]),
    ("data de abertura", &[ExportField::DataInicio]),
    ("data da abertura", &[ExportField::DataInicio]),
    ("data de fundacao", &[ExportField::DataInicio]),
    ("abertura", &[ExportField::DataInicio]),
    ("fundacao", &[ExportField::DataInicio]),
    ("cnae principal", &[ExportField::CnaePrincipal]),
    ("codigo cnae", &[ExportField::CnaePrincipal]),
    ("codigo da atividade", &[ExportField::CnaePrincipal]),
    ("cnaes", &[ExportField::CnaePrincipal]),
    ("cnae", &[ExportField::CnaePrincipal]),
    ("descricao da atividade", &[ExportField::CnaeDescricao]),
    ("atividade principal", &[ExportField::CnaeDescricao]),
    ("ramo de atividade", &[ExportField::CnaeDescricao]),
    ("atividade", &[ExportField::CnaeDescricao]),
    ("natureza juridica", &[ExportField::NaturezaJuridica]),
    ("natureza", &[ExportField::NaturezaJuridica]),
    ("porte", &[ExportField::Porte]),
    ("capital social", &[ExportField::CapitalSocial]),
    ("capital", &[ExportField::CapitalSocial]),
    ("opcao pelo simples", &[ExportField::Simples]),
    ("simples nacional", &[ExportField::Simples]),
    ("simples", &[ExportField::Simples]),
    ("opcao pelo mei", &[ExportField::Mei]),
    ("mei", &[ExportField::Mei]),
    ("endereco completo", &[ExportField::Endereco, ExportField::Bairro, ExportField::Cep]),
    ("enderecos", &[ExportField::Endereco]),
    ("endereco", &[ExportField::Endereco]),
    ("logradouro", &[ExportField::Endereco]),
    ("bairro", &[ExportField::Bairro]),
    ("cep", &[ExportField::Cep]),
    ("municipio", &[ExportField::Municipio]),
    ("cidade", &[ExportField::Municipio]),
    ("uf", &[ExportField::Uf]),
    ("estado", &[ExportField::Uf]),
    ("telefones", &[ExportField::Telefone]),
    ("telefone", &[ExportField::Telefone]),
    ("fone", &[ExportField::Telefone]),
    ("contatos", &[ExportField::Telefone, ExportField::Email]),
    ("contato", &[ExportField::Telefone, ExportField::Email]),
    ("e mail", &[ExportField::Email]),
    ("emails", &[ExportField::Email]),
    ("email", &[ExportField::Email]),
    ("correio eletronico", &[ExportField::Email]),
];

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

fn phrase_at(words: &[Word], at: usize, phrase: &str) -> Option<usize> {
    let needle: Vec<&str> = phrase.split(' ').collect();
    let end = at + needle.len();
    (end <= words.len() && words[at..end].iter().zip(&needle).all(|(w, n)| w.key == *n)).then_some(end)
}

fn format_at(words: &[Word], at: usize) -> Option<(usize, ExportFormat)> {
    FORMATS.iter().find_map(|(phrase, format)| phrase_at(words, at, phrase).map(|end| (end, *format)))
}

fn field_at(words: &[Word], at: usize) -> Option<(usize, &'static [ExportField], bool)> {
    FIELDS
        .iter()
        .find_map(|(phrase, fields)| phrase_at(words, at, phrase).map(|end| (end, *fields, matches!(*phrase, "numero" | "numeros"))))
}

struct FieldList {
    end: usize,
    fields: Vec<ExportField>,
    named: usize,
    bare_number: bool,
}

/// `[article] field ((, | e | , e) [article] field)*`, then one optional genitive.
fn field_list(words: &[Word], mut at: usize) -> Option<FieldList> {
    let mut list = FieldList { end: at, fields: Vec::new(), named: 0, bare_number: false };
    loop {
        let mut next = at;
        if words.get(next).is_some_and(|w| ARTICLES.contains(&w.key.as_str())) {
            next += 1;
        }
        let Some((end, fields, bare)) = field_at(words, next) else { break };
        for field in fields {
            if !list.fields.contains(field) {
                list.fields.push(*field);
            }
        }
        list.named += 1;
        list.bare_number |= bare;
        list.end = end;
        at = end;
        let comma = words[end - 1].comma;
        if words.get(at).is_some_and(|w| matches!(w.key.as_str(), "e" | "mais")) {
            at += 1;
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
        || format_at(words, list.end).is_some()
        || VERBS.contains(&words[list.end].key.as_str());
    if !boundary {
        return None;
    }
    if words.get(list.end).is_some_and(|w| GENITIVES.contains(&w.key.as_str())) {
        list.end += 1;
    }
    Some(list)
}

/// Extract a file delivery. `previous` is the export of the conversation, which a bare
/// column list ("só o nome e o CNPJ") refines.
pub(crate) fn delivery(text: &str, previous: Option<&Export>) -> Option<Delivery> {
    let words = words(text);
    let format = (0..words.len()).find_map(|at| format_at(&words, at).map(|(end, format)| (at, end, format)));
    let verb = words.iter().position(|w| EXPORT_VERBS.contains(&w.key.as_str()));
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
    let delivered = format.is_some() || verb.is_some();
    if !delivered && previous.is_none() {
        return None;
    }
    // Column lists: after an explicit introduction, or right after a request verb.
    let mut at = 0;
    while at < words.len() {
        let intro = FIELD_INTROS.iter().find_map(|phrase| phrase_at(&words, at, phrase));
        let after_verb = delivered && VERBS.contains(&words[at].key.as_str()) && !matches!(words[at].key.as_str(), "por" | "favor");
        if let Some(end) = intro.or(after_verb.then_some(at + 1))
            && let Some(list) = field_list(&words, end)
        {
            take(&list, if intro.is_some() { at } else { end }, &mut removed);
            at = list.end;
            continue;
        }
        at += 1;
    }
    if let Some((start, end, _)) = format {
        removed[start..end].iter_mut().for_each(|r| *r = true);
        // "com"/"contendo" right after the format introduces columns even for one name.
        let mut right = end;
        if let Some(word) = words.get(right)
            && RIGHT_GLUE.contains(&word.key.as_str())
        {
            if matches!(word.key.as_str(), "com" | "contendo")
                && let Some(list) = field_list(&words, right + 1)
            {
                take(&list, right, &mut removed);
                right = list.end;
            } else {
                removed[right] = true;
                right += 1;
            }
        }
        let _ = right;
        let mut left = start;
        while left > 0 {
            let word = &words[left - 1];
            if removed[left - 1] || LEFT_GLUE.contains(&word.key.as_str()) || VERBS.contains(&word.key.as_str()) {
                removed[left - 1] = true;
                left -= 1;
            } else {
                break;
            }
        }
    }
    if let Some(at) = verb {
        // "Exporte as farmácias…": the verb and the request words before it.
        let mut left = at + 1;
        while left > 0 && (removed[left - 1] || VERBS.contains(&words[left - 1].key.as_str())) {
            removed[left - 1] = true;
            left -= 1;
        }
        let mut right = at + 1;
        while words.get(right).is_some_and(|w| matches!(w.key.as_str(), "para" | "em" | "como")) && format.is_none() {
            removed[right] = true;
            right += 1;
        }
    }
    if !delivered && fields.is_empty() {
        return None;
    }
    // A request opening the question ("Por favor, me mande …") is cut as well.
    if delivered {
        for (word, removed) in words.iter().zip(removed.iter_mut()) {
            if matches!(word.key.as_str(), "agora" | "entao") || *removed {
                continue;
            }
            if !VERBS.contains(&word.key.as_str()) {
                break;
            }
            *removed = true;
        }
    }
    // Keep a token unless every word in it belongs to the delivery phrase.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let rest = tokens
        .iter()
        .enumerate()
        .filter(|(token, _)| words.iter().zip(&removed).any(|(w, r)| w.token == *token && !*r) || words.iter().all(|w| w.token != *token))
        .map(|(_, raw)| *raw)
        .collect::<Vec<_>>()
        .join(" ");
    let format = match (format, previous) {
        (Some((_, _, format)), _) => format,
        (None, Some(previous)) if !delivered => previous.format,
        _ => {
            notes.push("Gerei o arquivo em Excel (.xlsx). Peça CSV ou TXT se preferir outro formato.".into());
            ExportFormat::Xlsx
        }
    };
    if bare_number {
        notes.push("Considerei “número” como o número de inscrição no CNPJ. Para contatos, peça o telefone.".into());
    }
    // "Agora em CSV" changes the format of the previous file and keeps its columns.
    let rest = rest.trim_matches([',', ';', ' ']).to_owned();
    let fields = match previous {
        Some(previous) if fields.is_empty() && (!delivered || refers_to_previous(&rest)) => previous.fields.clone(),
        _ => fields,
    };
    Some(Delivery { export: Export { format, fields, rows: None }, rest, notes })
}

/// A delivery follow-up whose remaining words only point at the previous answer.
pub(crate) fn refers_to_previous(rest: &str) -> bool {
    normalize::words(rest).iter().all(|w| {
        VERBS.contains(&w.as_str())
            || matches!(
                w.as_str(),
                "agora"
                    | "isso"
                    | "isto"
                    | "esse"
                    | "essa"
                    | "esses"
                    | "essas"
                    | "este"
                    | "esta"
                    | "estes"
                    | "estas"
                    | "resultado"
                    | "resultados"
                    | "lista"
                    | "listagem"
                    | "relacao"
                    | "tabela"
                    | "dados"
                    | "mesmo"
                    | "mesma"
                    | "mesmos"
                    | "mesmas"
                    | "tambem"
                    | "e"
                    | "o"
                    | "a"
                    | "os"
                    | "as"
                    | "ai"
                    | "aqui"
                    | "entao"
                    | "agr"
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(text: &str) -> (Export, String) {
        let d = delivery(text, None).unwrap_or_else(|| panic!("no delivery in {text}"));
        (d.export, d.rest)
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
    fn data_words_are_not_columns() {
        let (export, rest) = cut("Exporte em CSV somente as matrizes ativas de padarias em Sorriso MT");
        assert!(export.fields.is_empty());
        assert_eq!(rest, "somente as matrizes ativas de padarias em Sorriso MT");
        assert!(delivery("quantas padarias ativas existem em Sorriso", None).is_none());
        assert!(delivery("quantas empresas tiveram baixa em 2025 em Sinop MT", None).is_none());
    }

    #[test]
    fn a_bare_column_list_refines_the_previous_export() {
        let previous = Export { format: ExportFormat::Csv, fields: Vec::new(), rows: None };
        let d = delivery("só o nome e o CNPJ", Some(&previous)).unwrap();
        assert_eq!(d.export.format, ExportFormat::Csv);
        assert_eq!(d.export.fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
        assert!(refers_to_previous(&d.rest));
        let d = delivery("agora em txt", Some(&previous)).unwrap();
        assert_eq!(d.export.format, ExportFormat::Txt);
        assert!(refers_to_previous(&d.rest));
    }
}
