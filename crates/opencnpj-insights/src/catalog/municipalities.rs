//! Municipality rows joined from RFB, Tesouro and IBGE sources.

use std::{collections::BTreeMap, fs, io::Write, path::Path};

use anyhow::{Context, Result, bail, ensure};

pub const CENSUS_UNAVAILABLE_IBGE: u32 = 5101837;
pub const FOREIGN_TOM: u16 = 9707;
const HEADER: &str = "id\ttom\tibge\tuf\tname\tfolded\tvariants\tpop_census_2022\tpop_est\tpop_est_year\tlat\tlon\tneighbours\treg_imediata\treg_intermediaria\tcapital";

#[derive(Debug, PartialEq)]
pub struct Municipality {
    pub id: u16,
    pub tom: u16,
    pub name: String,
    pub folded: String,
    pub variants: Vec<String>,
    pub place: Place,
}

#[derive(Debug, PartialEq)]
pub enum Place {
    Municipality(Geography),
    Abroad,
}

#[derive(Debug, PartialEq)]
pub struct Geography {
    pub ibge: u32,
    pub uf: String,
    pub census_2022: Option<u32>,
    pub population_estimate: u32,
    pub estimate_year: u16,
    pub latitude: f64,
    pub longitude: f64,
    pub neighbours: Vec<u16>,
    pub immediate_region: u32,
    pub intermediate_region: u16,
    pub capital: bool,
}

#[derive(Debug)]
pub struct MunicipalityCatalogue {
    rows: Vec<Municipality>,
    by_tom: BTreeMap<u16, u16>,
    by_ibge: BTreeMap<u32, u16>,
}

impl MunicipalityCatalogue {
    /// The population reference this catalogue holds by default: the latest IBGE estimate
    /// loaded, or the 2022 census when no estimate is present. Defaults must come from here,
    /// never from a hard-coded year.
    pub fn default_population(&self) -> opencnpj_query::ir::PopulationRef {
        self.rows()
            .iter()
            .filter_map(|row| match &row.place {
                Place::Municipality(geo) => Some(geo.estimate_year),
                Place::Abroad => None,
            })
            .max()
            .map_or(opencnpj_query::ir::PopulationRef::IbgeCensus(2022), opencnpj_query::ir::PopulationRef::IbgeEstimate)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let tsv = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut lines = tsv.lines();
        ensure!(lines.next() == Some(HEADER), "unexpected municipality catalogue header");
        let rows = lines
            .enumerate()
            .map(|(i, line)| parse_row(line).with_context(|| format!("municipality catalogue line {}", i + 2)))
            .collect::<Result<_>>()?;
        Self::from_rows(rows)
    }

    pub fn from_rows(rows: Vec<Municipality>) -> Result<Self> {
        ensure!(!rows.is_empty(), "empty municipality catalogue");
        let mut by_tom = BTreeMap::new();
        let mut by_ibge = BTreeMap::new();
        for (index, row) in rows.iter().enumerate() {
            ensure!(usize::from(row.id) == index, "non-dense municipality ID {}", row.id);
            ensure!(row.tom <= 9999 && by_tom.insert(row.tom, row.id).is_none(), "invalid or duplicate TOM code {}", row.tom);
            ensure!(valid_text(&row.name) && row.folded == fold_name(&row.name), "invalid name for TOM {:04}", row.tom);
            ensure!(
                row.variants.windows(2).all(|p| p[0] < p[1])
                    && row.variants.iter().all(|name| valid_text(name) && *name == fold_name(name) && *name != row.folded),
                "invalid variants for TOM {:04}",
                row.tom
            );
            if let Place::Municipality(geo) = &row.place {
                ensure!(row.tom != FOREIGN_TOM, "foreign TOM cannot map to an IBGE municipality");
                ensure!(
                    (1_000_000..=9_999_999).contains(&geo.ibge) && by_ibge.insert(geo.ibge, row.id).is_none(),
                    "invalid or duplicate IBGE code {}",
                    geo.ibge
                );
                ensure!(
                    geo.uf.len() == 2 && geo.uf.bytes().all(|b| b.is_ascii_uppercase()) && geo.uf != "EX",
                    "invalid UF for {}",
                    geo.ibge
                );
                ensure!(geo.estimate_year >= 2022 && geo.population_estimate > 0, "invalid estimate for {}", geo.ibge);
                ensure!(
                    geo.census_2022.map_or(geo.ibge == CENSUS_UNAVAILABLE_IBGE, |count| count > 0),
                    "missing or invalid census population for {}",
                    geo.ibge
                );
                ensure!(
                    (-90.0..=90.0).contains(&geo.latitude) && (-180.0..=180.0).contains(&geo.longitude),
                    "invalid coordinates for {}",
                    geo.ibge
                );
                ensure!(geo.immediate_region > 0 && geo.intermediate_region > 0, "missing regions for {}", geo.ibge);
                ensure!(geo.neighbours.windows(2).all(|p| p[0] < p[1]), "unordered or duplicate neighbours for {}", geo.ibge);
            } else {
                ensure!(row.tom == FOREIGN_TOM, "unexpected foreign TOM {:04}", row.tom);
            }
        }
        for row in &rows {
            if let Place::Municipality(geo) = &row.place {
                for &id in &geo.neighbours {
                    let reverse = rows.get(usize::from(id)).is_some_and(
                        |other| matches!(&other.place, Place::Municipality(geo) if geo.neighbours.binary_search(&row.id).is_ok()),
                    );
                    ensure!(id != row.id && reverse, "invalid neighbour link {} -> {id}", row.id);
                }
            }
        }
        Ok(Self { rows, by_tom, by_ibge })
    }

    pub fn rows(&self) -> &[Municipality] {
        &self.rows
    }

    pub fn get(&self, id: u16) -> Option<&Municipality> {
        self.rows.get(usize::from(id))
    }

    pub fn by_tom(&self, tom: u16) -> Option<&Municipality> {
        self.by_tom.get(&tom).and_then(|&id| self.get(id))
    }

    pub fn by_ibge(&self, ibge: u32) -> Option<&Municipality> {
        self.by_ibge.get(&ibge).and_then(|&id| self.get(id))
    }

    pub fn write(&self, mut writer: impl Write) -> Result<()> {
        writeln!(writer, "{HEADER}")?;
        for row in &self.rows {
            let variants = row.variants.join("|");
            write!(writer, "{}\t{:04}\t", row.id, row.tom)?;
            match &row.place {
                Place::Municipality(g) => {
                    let census = g.census_2022.map(|n| n.to_string()).unwrap_or_default();
                    let neighbours = g.neighbours.iter().map(u16::to_string).collect::<Vec<_>>().join(",");
                    writeln!(
                        writer,
                        "{}\t{}\t{}\t{}\t{variants}\t{census}\t{}\t{}\t{}\t{}\t{neighbours}\t{}\t{}\t{}",
                        g.ibge,
                        g.uf,
                        row.name,
                        row.folded,
                        g.population_estimate,
                        g.estimate_year,
                        g.latitude,
                        g.longitude,
                        g.immediate_region,
                        g.intermediate_region,
                        u8::from(g.capital),
                    )?;
                }
                Place::Abroad => writeln!(writer, "\tEX\t{}\t{}\t{variants}\t\t\t\t\t\t\t\t\t0", row.name, row.folded)?,
            }
        }
        writer.flush()?;
        Ok(())
    }
}

fn parse_row(line: &str) -> Result<Municipality> {
    let fields: Vec<_> = line.split('\t').collect();
    let [id, tom, ibge, uf, name, folded, variants, census, estimate, year, lat, lon, neighbours, immediate, intermediate, capital] =
        fields[..]
    else {
        bail!("expected 16 fields");
    };
    ensure!(tom.len() == 4 && tom.bytes().all(|b| b.is_ascii_digit()), "expected a four-digit TOM code");
    let capital = match capital {
        "0" => false,
        "1" => true,
        _ => bail!("expected capital flag 0 or 1"),
    };
    let place = if uf == "EX" {
        ensure!(
            [ibge, census, estimate, year, lat, lon, neighbours, immediate, intermediate].iter().all(|v| v.is_empty()) && !capital,
            "foreign row has municipal fields"
        );
        Place::Abroad
    } else {
        Place::Municipality(Geography {
            ibge: ibge.parse()?,
            uf: uf.into(),
            census_2022: (!census.is_empty()).then(|| census.parse()).transpose()?,
            population_estimate: estimate.parse()?,
            estimate_year: year.parse()?,
            latitude: lat.parse()?,
            longitude: lon.parse()?,
            neighbours: neighbours.split(',').filter(|_| !neighbours.is_empty()).map(str::parse).collect::<Result<_, _>>()?,
            immediate_region: immediate.parse()?,
            intermediate_region: intermediate.parse()?,
            capital,
        })
    };
    Ok(Municipality {
        id: id.parse()?,
        tom: tom.parse()?,
        name: name.into(),
        folded: folded.into(),
        variants: variants.split('|').filter(|_| !variants.is_empty()).map(str::to_owned).collect(),
        place,
    })
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && !value.contains('|') && !value.chars().any(char::is_control)
}

/// Fold Portuguese accents and case while preserving punctuation and word boundaries.
pub fn fold_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à'..='å' => 'a',
            'ç' => 'c',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ñ' => 'n',
            'ò'..='ö' | 'ø' => 'o',
            'ù'..='ü' => 'u',
            'ý' | 'ÿ' => 'y',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOA: &str =
        "0\t1182\t5101837\tMT\tBoa Esperança do Norte\tboa esperanca do norte\t\t\t5983\t2026\t-13.5\t-55.1\t\t510008\t5103\t0";

    #[test]
    fn census_exception_does_not_fill_the_census_from_an_estimate() {
        let row = parse_row(BOA).unwrap();
        let catalogue = MunicipalityCatalogue::from_rows(vec![row]).unwrap();
        let Place::Municipality(geo) = &catalogue.by_tom(1182).unwrap().place else { panic!("municipality") };
        assert_eq!(geo.census_2022, None);
        assert_eq!(geo.population_estimate, 5983);
        let mut bytes = Vec::new();
        catalogue.write(&mut bytes).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), format!("{HEADER}\n{BOA}\n"));

        let other = BOA.replace("5101837", "5107925");
        assert!(MunicipalityCatalogue::from_rows(vec![parse_row(&other).unwrap()]).is_err());
    }

    #[test]
    fn neighbour_links_require_existing_reverse_links() {
        let first = BOA.replace("\t\t510008", "\t1\t510008");
        let second = BOA.replace("0\t1182\t5101837", "1\t9907\t5107925").replace("\t\t5983", "\t10000\t5983");
        assert!(MunicipalityCatalogue::from_rows(vec![parse_row(&first).unwrap(), parse_row(&second).unwrap()]).is_err());
        let second = second.replace("\t\t510008", "\t0\t510008");
        assert!(MunicipalityCatalogue::from_rows(vec![parse_row(&first).unwrap(), parse_row(&second).unwrap()]).is_ok());
    }
}
