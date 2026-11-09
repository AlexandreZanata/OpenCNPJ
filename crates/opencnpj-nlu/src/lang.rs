//! Catalogue-backed language pack. Data is supplied by the host, never read per request.

use crate::spotter::{Entity, Lexicon};
use opencnpj_query::ir::*;
use std::collections::BTreeMap;

pub const UFS: [&str; 27] = [
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO", "RR",
    "RS", "SC", "SE", "SP", "TO",
];
const UF_NAMES: [&str; 27] = [
    "Acre",
    "Alagoas",
    "Amazonas",
    "Amapá",
    "Bahia",
    "Ceará",
    "Distrito Federal",
    "Espírito Santo",
    "Goiás",
    "Maranhão",
    "Minas Gerais",
    "Mato Grosso do Sul",
    "Mato Grosso",
    "Pará",
    "Paraíba",
    "Pernambuco",
    "Piauí",
    "Paraná",
    "Rio de Janeiro",
    "Rio Grande do Norte",
    "Rondônia",
    "Roraima",
    "Rio Grande do Sul",
    "Santa Catarina",
    "Sergipe",
    "São Paulo",
    "Tocantins",
];

pub struct LanguagePack {
    pub(crate) lexicon: Lexicon,
    pub(crate) municipality_ufs: BTreeMap<MunicipalityId, UfId>,
    pub(crate) sector_cnaes: BTreeMap<SectorId, SmallSet<CnaeCode>>,
}

impl Default for LanguagePack {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguagePack {
    pub fn new() -> Self {
        let mut lexicon = Lexicon::default();
        for (i, (code, name)) in UFS.iter().zip(UF_NAMES).enumerate() {
            lexicon.insert(code, Entity::Uf(UfId(i as u8)));
            lexicon.insert(name, Entity::Uf(UfId(i as u8)));
        }
        for (i, region) in ["Norte", "Nordeste", "Sudeste", "Sul", "Centro-Oeste"].iter().enumerate() {
            lexicon.insert(&format!("regiao {region}"), Entity::Region(RegionId(i as u16 + 1)));
            lexicon.insert(region, Entity::Region(RegionId(i as u16 + 1)));
        }
        Self { lexicon, municipality_ufs: BTreeMap::new(), sector_cnaes: BTreeMap::new() }
    }
    pub fn municipality(&mut self, id: MunicipalityId, uf: UfId, name: &str, aliases: &[String]) {
        self.municipality_ufs.insert(id, uf);
        self.lexicon.insert(name, Entity::Municipality { id, uf });
        for alias in aliases {
            self.lexicon.insert(alias, Entity::Municipality { id, uf });
        }
    }
    pub fn sector(&mut self, id: SectorId, aliases: &[String], codes: SmallSet<CnaeCode>) {
        self.sector_cnaes.insert(id, codes);
        for alias in aliases {
            self.lexicon.insert(alias, Entity::Sector(id));
        }
    }
}
