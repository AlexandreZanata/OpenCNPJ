//! Compact, versioned counts derived from a single verified snapshot.

pub mod builder;
mod format;

pub use format::{Aggregate, write};

use opencnpj_query::ir::{Competence, Day, MunicipalityId, UfId};
use serde::{Deserialize, Serialize};

pub const BLOCK_CELLS: usize = 4096;
pub const DETAIL_BITS: u8 = 17;
pub const FIRST_MONTHLY_YEAR: u16 = 2021;
pub const UF_MISSING: u8 = 31;
pub const MUNICIPALITY_MISSING: u16 = u16::MAX;

const UFS: &[&str] = &[
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO", "RR",
    "RS", "SC", "SE", "SP", "TO", "EX", "BR",
];

pub fn uf_id(code: &str) -> Option<UfId> {
    UFS.iter().position(|uf| *uf == code).map(|id| UfId(id as u8))
}

pub fn uf_code(id: UfId) -> Option<&'static str> {
    UFS.get(usize::from(id.0)).copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Table {
    T1 = 1,
    T2 = 2,
    T3 = 3,
    T4 = 4,
    T5 = 5,
}

impl Table {
    pub const ALL: [Self; 5] = [Self::T1, Self::T2, Self::T3, Self::T4, Self::T5];

    fn from_tag(tag: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|table| *table as u8 == tag)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    Cells(Table),
    Cnae(Table),
    Uf(Table),
    Municipality(Table),
    UfCnae(Table),
    CnaeOffset(Table),
}

impl Section {
    pub fn table(self) -> Table {
        match self {
            Self::Cells(table)
            | Self::Cnae(table)
            | Self::Uf(table)
            | Self::Municipality(table)
            | Self::UfCnae(table)
            | Self::CnaeOffset(table) => table,
        }
    }

    fn tag(self) -> u8 {
        let family = match self {
            Self::Cells(_) => 0,
            Self::Cnae(_) => 1,
            Self::Uf(_) => 2,
            Self::Municipality(_) => 3,
            Self::UfCnae(_) => 4,
            Self::CnaeOffset(_) => 5,
        };
        (family << 3) | self.table() as u8
    }

    fn from_tag(tag: u8) -> Option<Self> {
        let table = Table::from_tag(tag & 7)?;
        Some(match tag >> 3 {
            0 => Self::Cells(table),
            1 => Self::Cnae(table),
            2 => Self::Uf(table),
            3 => Self::Municipality(table),
            4 => Self::UfCnae(table),
            5 => Self::CnaeOffset(table),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub key: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// Exact raw subclass, or zero when the field is absent.
    pub cnae: u32,
    pub municipality: MunicipalityId,
    /// UF from the establishment, including EX and the missing sentinel.
    pub uf: UfId,
    pub detail: u32,
}

impl Key {
    pub fn pack(self) -> Option<u64> {
        if self.cnae > 9_999_999 || self.uf.0 > UF_MISSING || self.detail >= 1 << DETAIL_BITS {
            return None;
        }
        Some(self.packed())
    }

    fn packed(self) -> u64 {
        (u64::from(self.cnae) << 38) | (u64::from(self.municipality.0) << 22) | (u64::from(self.uf.0) << 17) | u64::from(self.detail)
    }

    pub fn unpack(value: u64) -> Self {
        Self {
            cnae: (value >> 38) as u32,
            municipality: MunicipalityId((value >> 22) as u16),
            uf: UfId(((value >> 17) & 31) as u8),
            detail: (value & ((1 << DETAIL_BITS) - 1)) as u32,
        }
    }

    pub fn uf_cnae(self) -> u64 {
        (u64::from(self.uf.0) << 24) | u64::from(self.cnae)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveDetail {
    pub head_office: u8,
    pub porte: u8,
    pub mei: u8,
    pub simples: u8,
}

impl ActiveDetail {
    pub fn pack(self) -> u32 {
        u32::from(self.head_office | (self.porte << 2)) | (u32::from(self.mei) << 5) | (u32::from(self.simples) << 7)
    }

    pub fn unpack(value: u32) -> Self {
        Self {
            head_office: (value & 3) as u8,
            porte: ((value >> 2) & 7) as u8,
            mei: ((value >> 5) & 3) as u8,
            simples: ((value >> 7) & 3) as u8,
        }
    }
}

pub fn stock_detail(head_office: u8, status: u8) -> u32 {
    u32::from(head_office | (status << 2))
}

pub fn unpack_stock_detail(value: u32) -> (u8, u8) {
    ((value & 3) as u8, ((value >> 2) & 7) as u8)
}

/// Year/month bucket stored with flows. Earlier years share their January bucket.
pub fn flow_bucket(year: u16, month: u8) -> u32 {
    1 + u32::from(year - 1900) * 12 + if year < FIRST_MONTHLY_YEAR { 0 } else { u32::from(month - 1) }
}

pub fn flow_bounds(bucket: u32) -> Option<(Day, Day)> {
    let zero = bucket.checked_sub(1)?;
    let year = u16::try_from(1900 + zero / 12).ok()?;
    let month = (zero % 12 + 1) as u8;
    if year < FIRST_MONTHLY_YEAR {
        return (month == 1).then_some((Day::from_ymd(year, 1, 1)?, Day::from_ymd(year, 12, 31)?));
    }
    let start = Day::from_ymd(year, month, 1)?;
    let end = match (year, month) {
        (9999, 12) => Day::from_ymd(year, 12, 31)?,
        (_, 12) => Day(Day::from_ymd(year + 1, 1, 1)?.0 - 1),
        _ => Day(Day::from_ymd(year, month + 1, 1)?.0 - 1),
    };
    Some((start, end))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub competence: Competence,
    pub snapshot_sha256: String,
    pub municipality_sha256: String,
    pub tables: Vec<Table>,
    pub establishments: u64,
    pub missing_registration_date: u64,
    pub missing_closure_date: u64,
}
