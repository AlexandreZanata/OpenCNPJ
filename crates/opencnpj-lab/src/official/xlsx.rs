//! Minimal read-only XLSX support for official tabular resources.
//!
//! Only the parts needed by the admitted datasets are implemented: shared
//! strings, sheet names and cell values. No macros, formulas or styles.

use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use zip::ZipArchive;

pub struct Workbook {
    path: PathBuf,
    shared: Vec<String>,
    sheets: BTreeMap<String, String>,
}

impl Workbook {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut zip = ZipArchive::new(File::open(&path).with_context(|| format!("open {}", path.display()))?)?;
        let shared = parse_shared_strings(&mut zip)?;
        let workbook = read_xml(&mut zip, "xl/workbook.xml")?;
        let relationships = read_xml(&mut zip, "xl/_rels/workbook.xml.rels")?;
        let mut targets = BTreeMap::new();
        let mut xml = Reader::from_str(&relationships);
        loop {
            match xml.read_event()? {
                Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"Relationship" => {
                    let id = attribute(&tag, b"Id", xml.decoder())?;
                    let target = attribute(&tag, b"Target", xml.decoder())?;
                    if let (Some(id), Some(target)) = (id, target) {
                        targets.insert(id, target);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let mut sheets = BTreeMap::new();
        let mut xml = Reader::from_str(&workbook);
        loop {
            match xml.read_event()? {
                Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"sheet" => {
                    let name = attribute(&tag, b"name", xml.decoder())?;
                    let id = attribute(&tag, b"r:id", xml.decoder())?;
                    if let (Some(name), Some(id)) = (name, id) {
                        let target = targets.get(&id).with_context(|| format!("missing relationship {id} for sheet {name}"))?;
                        let target = target.trim_start_matches('/');
                        let target = if target.starts_with("xl/") { target.to_string() } else { format!("xl/{target}") };
                        sheets.insert(name, target);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        ensure!(!sheets.is_empty(), "workbook has no sheets");
        Ok(Self { path, shared, sheets })
    }

    pub fn sheet_names(&self) -> impl Iterator<Item = &str> {
        self.sheets.keys().map(String::as_str)
    }

    /// Read one sheet as rows of cells. Cell references determine the column.
    pub fn rows(&self, sheet: &str) -> Result<Vec<Vec<String>>> {
        let target = self.sheets.get(sheet).with_context(|| format!("unknown sheet {sheet}"))?;
        let mut zip = ZipArchive::new(File::open(&self.path).with_context(|| format!("open {}", self.path.display()))?)?;
        let xml = read_xml(&mut zip, target)?;
        let mut reader = Reader::from_str(&xml);
        let mut rows = Vec::new();
        let mut row = Vec::new();
        let mut in_row = false;
        let mut cell_column = 0_usize;
        let mut cell_type = String::new();
        let mut value = String::new();
        let mut in_value = false;
        let mut in_inline = false;
        loop {
            match reader.read_event()? {
                Event::Start(tag) if tag.name().as_ref() == b"row" => {
                    in_row = true;
                    row.clear();
                }
                Event::Empty(tag) if tag.name().as_ref() == b"row" => rows.push(Vec::new()),
                Event::Start(tag) if in_row && tag.name().as_ref() == b"c" => {
                    cell_column =
                        attribute(&tag, b"r", reader.decoder())?.and_then(|reference| column_index(&reference)).unwrap_or(row.len());
                    cell_type = attribute(&tag, b"t", reader.decoder())?.unwrap_or_default();
                    value.clear();
                }
                Event::Start(tag) if in_row && tag.name().as_ref() == b"v" => in_value = true,
                Event::Start(tag) if in_row && tag.name().as_ref() == b"is" => in_inline = true,
                Event::Text(text) if in_value => value.push_str(&text.decode()?),
                Event::Text(text) if in_inline => value.push_str(&text.decode()?),
                Event::End(tag) if in_row && tag.name().as_ref() == b"v" => in_value = false,
                Event::End(tag) if in_row && tag.name().as_ref() == b"c" => {
                    let decoded = if cell_type == "s" {
                        let index: usize = value.parse().with_context(|| format!("invalid shared string index {value}"))?;
                        self.shared.get(index).cloned().with_context(|| format!("shared string {index} out of range"))?
                    } else {
                        value.clone()
                    };
                    if row.len() <= cell_column {
                        row.resize(cell_column + 1, String::new());
                    }
                    row[cell_column] = decoded;
                }
                Event::End(tag) if in_row && tag.name().as_ref() == b"row" => {
                    rows.push(std::mem::take(&mut row));
                    in_row = false;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(rows)
    }
}

fn read_xml(zip: &mut ZipArchive<File>, name: &str) -> Result<String> {
    let mut file = zip.by_name(name).with_context(|| format!("missing {name} in workbook"))?;
    let mut text = String::new();
    file.read_to_string(&mut text).with_context(|| format!("read {name}"))?;
    Ok(text)
}

fn parse_shared_strings(zip: &mut ZipArchive<File>) -> Result<Vec<String>> {
    if zip.by_name("xl/sharedStrings.xml").is_err() {
        return Ok(Vec::new());
    }
    let xml = read_xml(zip, "xl/sharedStrings.xml")?;
    let mut reader = Reader::from_str(&xml);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"si" => current.clear(),
            Event::Start(tag) if tag.name().as_ref() == b"t" => in_text = true,
            Event::Text(text) if in_text => current.push_str(&text.decode()?),
            Event::End(tag) if tag.name().as_ref() == b"t" => in_text = false,
            Event::End(tag) if tag.name().as_ref() == b"si" => out.push(std::mem::take(&mut current)),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn attribute(tag: &BytesStart<'_>, key: &[u8], decoder: quick_xml::encoding::Decoder) -> Result<Option<String>> {
    Ok(tag
        .try_get_attribute(key)?
        .map(|value| value.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder).map(|value| value.into_owned()))
        .transpose()?)
}

/// Convert `AA12` to a zero-based column index.
fn column_index(reference: &str) -> Option<usize> {
    let letters = reference.chars().take_while(|character| character.is_ascii_alphabetic());
    let mut index = 0_usize;
    let mut any = false;
    for character in letters {
        any = true;
        index = index.checked_mul(26)?.checked_add(u32::from(character.to_ascii_uppercase()) as usize - u32::from('A') as usize + 1)?;
    }
    if any { index.checked_sub(1) } else { None }
}

pub fn cell(row: &[String], column: usize) -> &str {
    row.get(column).map(String::as_str).unwrap_or("")
}

pub fn parse_u32(value: &str) -> Result<u32> {
    let value = value.trim().replace('.', "");
    if value.is_empty() {
        bail!("empty integer")
    }
    value.parse().with_context(|| format!("parse integer {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_references() {
        assert_eq!(column_index("A1"), Some(0));
        assert_eq!(column_index("E16"), Some(4));
        assert_eq!(column_index("AA1"), Some(26));
        assert_eq!(column_index("1"), None);
    }
}
