//! Files delivered with an answer: Excel (.xlsx), CSV and plain text.
//!
//! The three writers share one [`Sheet`]: typed cells, so a spreadsheet keeps numbers and
//! dates sortable while the text formats print them in Brazilian notation. CSV follows the
//! RFB dialect (`;`, UTF-8 with BOM so Excel detects the encoding); the workbook is written
//! by hand as a minimal Office Open XML package.

use std::{
    collections::HashMap,
    io::{self, Seek, Write},
};

use opencnpj_core::{model::fixed_label, schema::col};
use opencnpj_query::{
    export::{ExportField, ExportFormat},
    ir::Day,
};
use opencnpj_store::{DocRecord, Snapshot};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::render::mask_cpf;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    /// A count.
    Integer(i64),
    /// A ratio or percentage, shown with one decimal place.
    Decimal(f64),
    /// Reais, shown with two decimal places.
    Money(f64),
    Date(Day),
}

impl Cell {
    /// The value in Brazilian notation, as the text formats print it.
    pub fn display(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(text) => text.clone(),
            Self::Integer(value) => group(&value.unsigned_abs().to_string(), *value < 0),
            Self::Decimal(value) => decimal(*value, 1),
            Self::Money(value) => decimal(*value, 2),
            Self::Date(day) => day.to_ymd().map_or_else(String::new, |(y, m, d)| format!("{d:02}/{m:02}/{y:04}")),
        }
    }
}

/// One table and the facts needed to read it out of context.
#[derive(Debug, Clone, Default)]
pub struct Sheet {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// Question, source, competence, filters and totals, in display order.
    pub about: Vec<(String, String)>,
}

pub fn write(format: ExportFormat, sheet: &Sheet, out: &mut (impl Write + Seek)) -> io::Result<()> {
    match format {
        ExportFormat::Xlsx => write_xlsx(sheet, out),
        ExportFormat::Csv => write_csv(sheet, out),
        ExportFormat::Txt => write_txt(sheet, out),
    }
}

/// `;`-separated, CRLF, UTF-8 with BOM: opens as columns in a Brazilian Excel.
pub fn write_csv(sheet: &Sheet, out: &mut impl Write) -> io::Result<()> {
    let mut out = io::BufWriter::new(out);
    out.write_all("\u{feff}".as_bytes())?;
    let line = |cells: &mut dyn Iterator<Item = String>| cells.map(|c| csv_field(&c)).collect::<Vec<_>>().join(";");
    write!(out, "{}\r\n", line(&mut sheet.columns.iter().cloned()))?;
    for row in &sheet.rows {
        write!(out, "{}\r\n", line(&mut row.iter().map(Cell::display)))?;
    }
    out.flush()
}

fn csv_field(value: &str) -> String {
    if value.contains([';', '"', '\n', '\r']) || value.starts_with(' ') || value.ends_with(' ') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// A report: the facts first, then the table with aligned columns.
pub fn write_txt(sheet: &Sheet, out: &mut impl Write) -> io::Result<()> {
    let mut out = io::BufWriter::new(out);
    writeln!(out, "{}", sheet.title)?;
    writeln!(out, "{}", "=".repeat(sheet.title.chars().count().max(3)))?;
    let key_width = sheet.about.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    for (key, value) in &sheet.about {
        writeln!(out, "{key:<key_width$}  {value}")?;
    }
    writeln!(out)?;
    let cells: Vec<Vec<String>> = sheet.rows.iter().map(|row| row.iter().map(Cell::display).collect()).collect();
    let mut widths: Vec<usize> = sheet.columns.iter().map(|c| c.chars().count()).collect();
    for row in &cells {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let numeric: Vec<bool> = (0..sheet.columns.len())
        .map(|i| sheet.rows.iter().any(|r| matches!(r.get(i), Some(Cell::Integer(_) | Cell::Decimal(_) | Cell::Money(_)))))
        .collect();
    let line = |row: &[String]| {
        row.iter()
            .zip(&widths)
            .zip(&numeric)
            .map(|((cell, width), right)| if *right { format!("{cell:>width$}") } else { format!("{cell:<width$}") })
            .collect::<Vec<_>>()
            .join(" | ")
            .trim_end()
            .to_owned()
    };
    writeln!(out, "{}", line(&sheet.columns))?;
    writeln!(out, "{}", widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("-+-"))?;
    for row in &cells {
        writeln!(out, "{}", line(row))?;
    }
    out.flush()
}

// Cell styles in `styles.xml`.
const STYLE_HEADER: u8 = 1;
const STYLE_MONEY: u8 = 2;
const STYLE_DATE: u8 = 3;
const STYLE_INTEGER: u8 = 4;
const STYLE_DECIMAL: u8 = 5;
const STYLE_LABEL: u8 = 6;

/// A two-sheet workbook: "Dados" (frozen header, filters, typed cells) and "Sobre".
pub fn write_xlsx(sheet: &Sheet, out: &mut (impl Write + Seek)) -> io::Result<()> {
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let (last_column, last_row) = (column_name(sheet.columns.len().max(1) - 1), sheet.rows.len() + 1);
    let last = format!("{last_column}{last_row}");
    let files: [(&str, String); 6] = [
        ("[Content_Types].xml", CONTENT_TYPES.into()),
        ("_rels/.rels", ROOT_RELS.into()),
        ("docProps/core.xml", core_properties(&sheet.title)),
        (
            "xl/workbook.xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Dados" sheetId="1" r:id="rId1"/><sheet name="Sobre" sheetId="2" r:id="rId2"/></sheets><definedNames><definedName name="_xlnm._FilterDatabase" localSheetId="0" hidden="1">Dados!$A$1:${last_column}${last_row}</definedName></definedNames></workbook>"#
            ),
        ),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS.into()),
        ("xl/styles.xml", STYLES.into()),
    ];
    for (name, body) in files {
        zip.start_file(name, options).map_err(io::Error::other)?;
        zip.write_all(body.as_bytes())?;
    }

    zip.start_file("xl/worksheets/sheet1.xml", options).map_err(io::Error::other)?;
    let widths = column_widths(sheet);
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols>"#
    )?;
    for (i, width) in widths.iter().enumerate() {
        write!(zip, r#"<col min="{0}" max="{0}" width="{width:.1}" customWidth="1"/>"#, i + 1)?;
    }
    zip.write_all(b"</cols><sheetData>")?;
    write_row(&mut zip, 1, sheet.columns.iter().map(|c| Cell::Text(c.clone())), Some(STYLE_HEADER))?;
    for (i, row) in sheet.rows.iter().enumerate() {
        write_row(&mut zip, i + 2, row.iter().cloned(), None)?;
    }
    write!(zip, r#"</sheetData><autoFilter ref="A1:{last}"/></worksheet>"#)?;

    zip.start_file("xl/worksheets/sheet2.xml", options).map_err(io::Error::other)?;
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="{MAIN}" xmlns:r="{REL}"><cols><col min="1" max="1" width="28" customWidth="1"/><col min="2" max="2" width="100" customWidth="1"/></cols><sheetData>"#
    )?;
    write_row(&mut zip, 1, [Cell::Text(sheet.title.clone())].into_iter(), Some(STYLE_LABEL))?;
    for (i, (key, value)) in sheet.about.iter().enumerate() {
        write!(zip, r#"<row r="{}">"#, i + 3)?;
        write_cell(&mut zip, &format!("A{}", i + 3), &Cell::Text(key.clone()), Some(STYLE_LABEL))?;
        write_cell(&mut zip, &format!("B{}", i + 3), &Cell::Text(value.clone()), None)?;
        zip.write_all(b"</row>")?;
    }
    zip.write_all(b"</sheetData></worksheet>")?;
    zip.finish().map_err(io::Error::other)?;
    Ok(())
}

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#;

const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

// Built-in number formats: 3 = #,##0; 4 = #,##0.00; 14 = short date (shown in the
// reader's locale, dd/mm/aaaa in pt-BR); 164 = one decimal place.
const STYLES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.0"/></numFmts><fonts count="3"><font><sz val="11"/><name val="Calibri"/><family val="2"/></font><font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Calibri"/><family val="2"/></font><font><b/><sz val="11"/><name val="Calibri"/><family val="2"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF1F4E79"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="7"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/><xf numFmtId="4" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="3" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"##;

fn core_properties(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:creator>OpenCNPJ</dc:creator></cp:coreProperties>"#,
        xml_text(title)
    )
}

fn write_row(out: &mut impl Write, row: usize, cells: impl Iterator<Item = Cell>, style: Option<u8>) -> io::Result<()> {
    write!(out, r#"<row r="{row}">"#)?;
    for (i, cell) in cells.enumerate() {
        write_cell(out, &format!("{}{row}", column_name(i)), &cell, style)?;
    }
    out.write_all(b"</row>")
}

fn write_cell(out: &mut impl Write, at: &str, cell: &Cell, style: Option<u8>) -> io::Result<()> {
    let number = |out: &mut dyn Write, value: f64, own: u8| {
        if value.is_finite() { write!(out, r#"<c r="{at}" s="{}"><v>{value}</v></c>"#, style.unwrap_or(own)) } else { Ok(()) }
    };
    match cell {
        Cell::Empty => Ok(()),
        Cell::Text(text) if text.is_empty() => Ok(()),
        Cell::Text(text) => {
            let space = if text.trim() != text { r#" xml:space="preserve""# } else { "" };
            let s = style.map(|s| format!(r#" s="{s}""#)).unwrap_or_default();
            write!(out, r#"<c r="{at}" t="inlineStr"{s}><is><t{space}>{}</t></is></c>"#, xml_text(text))
        }
        Cell::Integer(value) => number(out, *value as f64, STYLE_INTEGER),
        Cell::Decimal(value) => number(out, *value, STYLE_DECIMAL),
        Cell::Money(value) => number(out, *value, STYLE_MONEY),
        Cell::Date(day) => match excel_serial(*day) {
            Some(serial) => number(out, f64::from(serial), STYLE_DATE),
            None => write_cell(out, at, &Cell::Text(cell.display()), style),
        },
    }
}

/// Excel's serial day: 1 is 1900-01-01 and its phantom 1900-02-29 shifts later dates by one.
fn excel_serial(day: Day) -> Option<u32> {
    let march = Day::from_ymd(1900, 3, 1)?;
    (day.0 >= march.0 && day.to_ymd().is_some()).then_some(day.0 + 1)
}

fn column_name(mut index: usize) -> String {
    let mut name = Vec::new();
    loop {
        name.push(b'A' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    name.reverse();
    String::from_utf8(name).expect("ASCII column name")
}

/// Text allowed in XML 1.0, escaped.
fn xml_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || matches!(c, '\u{fffe}' | '\u{ffff}') => {}
            c => out.push(c),
        }
    }
    out
}

fn column_widths(sheet: &Sheet) -> Vec<f64> {
    (0..sheet.columns.len())
        .map(|i| {
            let longest = sheet
                .rows
                .iter()
                .take(2000)
                .filter_map(|row| row.get(i))
                .map(|cell| cell.display().chars().count())
                .chain([sheet.columns[i].chars().count()])
                .max()
                .unwrap_or(8);
            (longest as f64 + 2.0).clamp(8.0, 60.0)
        })
        .collect()
}

/// `1234567` -> `1.234.567`.
fn group(digits: &str, negative: bool) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    if negative { format!("-{out}") } else { out }
}

fn decimal(value: f64, places: usize) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let text = format!("{:.places$}", value.abs());
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let whole = group(whole, value < 0.0 && text.chars().any(|c| matches!(c, '1'..='9')));
    if fraction.is_empty() { whole } else { format!("{whole},{fraction}") }
}

/// Names that the snapshot spells without accents, keyed by the RFB municipality code.
#[derive(Debug, Clone, Default)]
pub struct RecordLabels {
    pub municipalities: HashMap<String, String>,
}

/// One establishment as export cells. Owner CPFs embedded in MEI/EI names are masked.
pub fn record_cells(snapshot: &Snapshot, labels: &RecordLabels, record: &DocRecord, fields: &[ExportField]) -> Vec<Cell> {
    let company = &record.company;
    let est = record.estab.and_then(|i| company.estabelecimentos.get(i));
    let emp = company.empresas.first();
    let sim = company.simples.first();
    let e = |i: usize| est.and_then(|row| row.get(i)).map_or("", |s| s.trim());
    let text = |value: &str| if value.is_empty() { Cell::Empty } else { Cell::Text(value.to_owned()) };
    let yes_no = |value: Option<&String>| match value.map(String::as_str) {
        Some("S") => Cell::Text("Sim".into()),
        Some("N") => Cell::Text("Não".into()),
        _ => Cell::Empty,
    };
    fields
        .iter()
        .map(|field| match field {
            ExportField::Cnpj => Cell::Text(format_cnpj(&format!("{}{}{}", company.basico, e(col::est::ORDEM), e(col::est::DV)))),
            ExportField::RazaoSocial => text(&mask_cpf(emp.map_or("", |row| row[col::emp::RAZAO_SOCIAL].trim()))),
            ExportField::NomeFantasia => text(&mask_cpf(e(col::est::NOME_FANTASIA))),
            ExportField::MatrizFilial => text(&title(fixed_label("identificador_matriz_filial", e(col::est::MATRIZ_FILIAL)).unwrap_or(""))),
            ExportField::Situacao => {
                text(&title(fixed_label("situacao_cadastral", e(col::est::SITUACAO)).unwrap_or(e(col::est::SITUACAO))))
            }
            ExportField::DataSituacao => date(e(col::est::DATA_SITUACAO)),
            ExportField::MotivoSituacao => {
                let code = e(col::est::MOTIVO);
                text(&snapshot.label("motivos", code).map_or_else(|| code.to_owned(), title))
            }
            ExportField::DataInicio => date(e(col::est::DATA_INICIO)),
            ExportField::CnaePrincipal => text(&format_cnae(e(col::est::CNAE_PRINCIPAL))),
            ExportField::CnaeDescricao => text(snapshot.label("cnaes", e(col::est::CNAE_PRINCIPAL)).unwrap_or("")),
            ExportField::NaturezaJuridica => {
                let code = emp.map_or("", |row| row[col::emp::NATUREZA].trim());
                text(&snapshot.label("naturezas", code).map_or_else(|| code.to_owned(), |label| format!("{code} - {label}")))
            }
            ExportField::Porte => text(match emp.map_or("", |row| row[col::emp::PORTE].trim()) {
                "00" => "Não informado",
                "01" => "Microempresa",
                "03" => "Empresa de pequeno porte",
                "05" => "Demais",
                _ => "",
            }),
            ExportField::CapitalSocial => emp
                .and_then(|row| row[col::emp::CAPITAL].trim().replace('.', "").replace(',', ".").parse::<f64>().ok())
                .map_or(Cell::Empty, Cell::Money),
            ExportField::Simples => yes_no(sim.map(|row| &row[col::sim::OPCAO_SIMPLES])),
            ExportField::Mei => yes_no(sim.map(|row| &row[col::sim::OPCAO_MEI])),
            ExportField::Endereco => {
                let street = [e(col::est::TIPO_LOGRADOURO), e(col::est::LOGRADOURO)].join(" ").trim().to_owned();
                let number = e(col::est::NUMERO);
                let complement = e(col::est::COMPLEMENTO);
                let mut address = street;
                if !number.is_empty() {
                    address = format!("{address}, {number}");
                }
                if !complement.is_empty() {
                    address = format!("{address} - {complement}");
                }
                text(address.trim_start_matches([',', ' ']))
            }
            ExportField::Bairro => text(e(col::est::BAIRRO)),
            ExportField::Cep => {
                let cep = e(col::est::CEP);
                text(&if cep.len() == 8 { format!("{}-{}", &cep[..5], &cep[5..]) } else { cep.to_owned() })
            }
            ExportField::Municipio => {
                let code = e(col::est::MUNICIPIO);
                text(
                    &labels.municipalities.get(code).cloned().or_else(|| snapshot.label("municipios", code).map(title)).unwrap_or_default(),
                )
            }
            ExportField::Uf => text(e(col::est::UF)),
            ExportField::Telefone => {
                let phones: Vec<String> = [(col::est::DDD_1, col::est::TELEFONE_1), (col::est::DDD_2, col::est::TELEFONE_2)]
                    .iter()
                    .filter(|(_, number)| !e(*number).is_empty())
                    .map(|(ddd, number)| format_phone(e(*ddd), e(*number)))
                    .collect();
                text(&phones.join("; "))
            }
            ExportField::Email => text(&e(col::est::EMAIL).to_lowercase()),
        })
        .collect()
}

fn date(value: &str) -> Cell {
    if value.len() != 8 || value.bytes().any(|b| !b.is_ascii_digit()) {
        return Cell::Empty;
    }
    let (y, m, d) = (value[..4].parse().ok(), value[4..6].parse().ok(), value[6..].parse().ok());
    match (y, m, d) {
        (Some(y), Some(m), Some(d)) => Day::from_ymd(y, m, d).map_or_else(|| Cell::Text(value.to_owned()), Cell::Date),
        _ => Cell::Empty,
    }
}

/// `12345678000190` -> `12.345.678/0001-90`; other lengths are kept as published.
pub fn format_cnpj(id: &str) -> String {
    if id.len() != 14 || !id.is_ascii() {
        return id.to_owned();
    }
    format!("{}.{}.{}/{}-{}", &id[..2], &id[2..5], &id[5..8], &id[8..12], &id[12..])
}

/// `6201501` -> `6201-5/01`.
fn format_cnae(code: &str) -> String {
    if code.len() == 7 && code.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}/{}", &code[..4], &code[4..5], &code[5..])
    } else {
        code.to_owned()
    }
}

fn format_phone(ddd: &str, number: &str) -> String {
    let split = number.len().saturating_sub(4);
    let number = if number.len() >= 8 { format!("{}-{}", &number[..split], &number[split..]) } else { number.to_owned() };
    if ddd.is_empty() { number } else { format!("({ddd}) {number}") }
}

/// RFB labels are upper case: "MICRO EMPRESA" -> "Micro empresa".
fn title(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    fn sheet() -> Sheet {
        Sheet {
            title: "Padarias & confeitarias".into(),
            columns: vec!["CNPJ".into(), "Nome".into(), "Capital social (R$)".into(), "Início".into(), "Total".into()],
            rows: vec![
                vec![
                    Cell::Text("12.345.678/0001-90".into()),
                    Cell::Text("PADARIA \"PAO; QUENTE\"".into()),
                    Cell::Money(1234567.5),
                    Cell::Date(Day::from_ymd(2025, 3, 9).unwrap()),
                    Cell::Integer(1234),
                ],
                vec![Cell::Text("98.765.432/0001-10".into()), Cell::Text("<SEM NOME>".into()), Cell::Empty, Cell::Empty, Cell::Integer(-5)],
            ],
            about: vec![("Pergunta".into(), "padarias em Sorriso".into())],
        }
    }

    #[test]
    fn csv_is_excel_friendly_and_lossless() {
        let mut out = Vec::new();
        write_csv(&sheet(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with('\u{feff}'));
        let lines: Vec<&str> = text.trim_start_matches('\u{feff}').split("\r\n").collect();
        assert_eq!(lines[0], "CNPJ;Nome;Capital social (R$);Início;Total");
        assert_eq!(lines[1], "12.345.678/0001-90;\"PADARIA \"\"PAO; QUENTE\"\"\";1.234.567,50;09/03/2025;1.234");
        assert_eq!(lines[2], "98.765.432/0001-10;<SEM NOME>;;;-5");
    }

    #[test]
    fn txt_aligns_columns_under_the_facts() {
        let mut out = Vec::new();
        write_txt(&sheet(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("Padarias & confeitarias\n"));
        assert!(text.contains("Pergunta  padarias em Sorriso"));
        assert!(text.contains("        1.234.567,50 | 09/03/2025 |"));
    }

    #[test]
    fn xlsx_is_a_valid_typed_package() {
        let mut out = Cursor::new(Vec::new());
        write_xlsx(&sheet(), &mut out).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(out.into_inner())).unwrap();
        let mut names: Vec<String> = archive.file_names().map(|name| name.unwrap().into_owned()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "[Content_Types].xml",
                "_rels/.rels",
                "docProps/core.xml",
                "xl/_rels/workbook.xml.rels",
                "xl/styles.xml",
                "xl/workbook.xml",
                "xl/worksheets/sheet1.xml",
                "xl/worksheets/sheet2.xml"
            ]
        );
        let mut data = String::new();
        archive.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut data).unwrap();
        assert!(data.contains(r#"<c r="C2" s="2"><v>1234567.5</v></c>"#));
        // 2025-03-09 is Excel serial 45725.
        assert!(data.contains(r#"<c r="D2" s="3"><v>45725</v></c>"#));
        assert!(data.contains("&lt;SEM NOME&gt;"));
        assert!(data.contains(r#"<autoFilter ref="A1:E3"/>"#));
        let mut workbook = String::new();
        archive.by_name("xl/workbook.xml").unwrap().read_to_string(&mut workbook).unwrap();
        assert!(workbook.contains("Dados!$A$1:$E$3"));
    }

    #[test]
    fn identifiers_and_numbers_use_brazilian_notation() {
        assert_eq!(format_cnpj("12345678000190"), "12.345.678/0001-90");
        assert_eq!(format_cnae("6201501"), "6201-5/01");
        assert_eq!(format_phone("65", "33224455"), "(65) 3322-4455");
        assert_eq!(column_name(0), "A");
        assert_eq!(column_name(25), "Z");
        assert_eq!(column_name(26), "AA");
        assert_eq!(decimal(-0.04, 1), "0,0");
        assert_eq!(Cell::Decimal(12345.678).display(), "12.345,7");
    }
}
