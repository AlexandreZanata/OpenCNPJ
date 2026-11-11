use opencnpj_core::varint::{self, Reader};

use crate::ir::*;

pub const IR_VERSION: u8 = 2;
const BASE_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("unsupported query IR version {0}")]
    Version(u8),
    #[error("truncated, malformed or noncanonical query IR")]
    Malformed,
}

impl From<varint::Malformed> for DecodeError {
    fn from(_: varint::Malformed) -> Self {
        Self::Malformed
    }
}

/// Encode the request losslessly. Canonicalize first when comparing meanings.
pub fn encode(request: &Request) -> Vec<u8> {
    let mut bytes = vec![if request.scope.name_prefix.is_some() { IR_VERSION } else { BASE_VERSION }];
    request.write(&mut bytes);
    if let Some(name) = &request.scope.name_prefix {
        name.field.write(&mut bytes);
        varint::put_lp(&mut bytes, name.prefix.as_bytes());
    }
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<Request, DecodeError> {
    let mut cursor = Cursor { reader: Reader::new(bytes), len: bytes.len() };
    let version = cursor.reader.u8()?;
    if !matches!(version, BASE_VERSION | IR_VERSION) {
        return Err(DecodeError::Version(version));
    }
    let mut request = Request::read(&mut cursor)?;
    if version == IR_VERSION {
        let field = NameField::read(&mut cursor)?;
        let prefix = std::str::from_utf8(cursor.reader.lp_bytes()?).map_err(|_| DecodeError::Malformed)?;
        request.scope.name_prefix = Some(NamePrefix { field, prefix: prefix.to_owned() });
    }
    // Re-encoding also rejects non-minimal and overflowing varints and
    // noncanonical set encodings. There is one byte sequence per value.
    if !cursor.reader.is_empty() || encode(&request) != bytes {
        return Err(DecodeError::Malformed);
    }
    Ok(request)
}

/// Hash a canonical query and the selected aggregate identity. Catalogue
/// versions are part of the canonical query. Display order does not affect the key.
pub fn cache_key(canonical: &Request, aggregates_id: &[u8]) -> u128 {
    let mut request = canonical.clone();
    request.output.display_order.clear();
    let mut bytes = encode(&request);
    varint::put_lp(&mut bytes, aggregates_id);
    xxhash_rust::xxh3::xxh3_128(&bytes)
}

struct Cursor<'a> {
    reader: Reader<'a>,
    len: usize,
}

trait Wire: Sized {
    fn write(&self, out: &mut Vec<u8>);
    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError>;
}

macro_rules! integers {
    ($($ty:ty),+) => {$ (
        impl Wire for $ty {
            fn write(&self, out: &mut Vec<u8>) { varint::put(out, u64::from(*self)); }
            fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
                Self::try_from(input.reader.varint()?).map_err(|_| DecodeError::Malformed)
            }
        }
    )+};
}

integers!(u8, u16, u32, u64);

impl Wire for bool {
    fn write(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        match input.reader.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Malformed),
        }
    }
}

impl<T: Wire> Wire for Option<T> {
    fn write(&self, out: &mut Vec<u8>) {
        self.is_some().write(out);
        if let Some(value) = self {
            value.write(out);
        }
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        bool::read(input)?.then(|| T::read(input)).transpose()
    }
}

fn write_slice<T: Wire>(values: &[T], out: &mut Vec<u8>) {
    varint::put(out, values.len() as u64);
    for value in values {
        value.write(out);
    }
}

fn read_values<T: Wire>(input: &mut Cursor<'_>) -> Result<impl Iterator<Item = Result<T, DecodeError>>, DecodeError> {
    let count = input.reader.varint_usize()?;
    if count > input.len - input.reader.position() {
        return Err(DecodeError::Malformed);
    }
    Ok((0..count).map(move |_| T::read(input)))
}

impl<T: Wire> Wire for Vec<T> {
    fn write(&self, out: &mut Vec<u8>) {
        write_slice(self, out);
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        read_values(input)?.collect()
    }
}

impl<T: Ord + Wire> Wire for SmallSet<T> {
    fn write(&self, out: &mut Vec<u8>) {
        write_slice(self, out);
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        read_values(input)?.collect()
    }
}

impl<T: Wire> Wire for Range<T> {
    fn write(&self, out: &mut Vec<u8>) {
        self.from.write(out);
        self.to.write(out);
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        Ok(Self { from: Wire::read(input)?, to: Wire::read(input)? })
    }
}

impl Wire for CnpjRef {
    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.as_str().len() as u8);
        out.extend_from_slice(self.as_str().as_bytes());
    }

    fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        let len = input.reader.u8()?;
        if !matches!(len, 8 | 14) {
            return Err(DecodeError::Malformed);
        }
        let bytes = input.reader.bytes(usize::from(len))?;
        let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::Malformed)?;
        Self::parse(text).map_err(|_| DecodeError::Malformed)
    }
}

macro_rules! id_wire {
    ($($type:ty),+) => {$ (
        impl Wire for $type {
            fn write(&self, out: &mut Vec<u8>) { self.0.write(out); }
            fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> { Ok(Self(Wire::read(input)?)) }
        }
    )+};
}

id_wire!(MunicipalityId, UfId, RegionId, SectorId, Competence, Day);

macro_rules! set_wire {
    ($($type:ty),+) => {$ (
        impl Wire for $type {
            fn write(&self, out: &mut Vec<u8>) { out.push(self.bits()); }
            fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
                Self::from_bits(input.reader.u8()?).ok_or(DecodeError::Malformed)
            }
        }
    )+};
}

set_wire!(StatusSet, PorteSet);

macro_rules! record {
    ($type:ty { $($field:ident),+ $(,)? } $(defaults {$($default:ident),+})?) => {
        impl Wire for $type {
            fn write(&self, out: &mut Vec<u8>) { $(self.$field.write(out);)+ }
            fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
                Ok(Self { $($field: Wire::read(input)?,)+ $($($default: Default::default(),)+)? })
            }
        }
    };
}

macro_rules! read_field {
    ($input:ident, $field:ident) => {
        Wire::read($input)?
    };
}

macro_rules! tagged {
    ($type:ty { $($tag:literal => $variant:ident $(($($value:ident),+))? $({$($field:ident),+})?),+ $(,)? }) => {
        impl Wire for $type {
            fn write(&self, out: &mut Vec<u8>) {
                match self { $(Self::$variant $(($($value),+))? $({$($field),+})? => {
                    out.push($tag);
                    $($($value.write(out);)+)?
                    $($($field.write(out);)+)?
                }),+ }
            }
            fn read(input: &mut Cursor<'_>) -> Result<Self, DecodeError> {
                Ok(match input.reader.u8()? {
                    $($tag => Self::$variant $(($(read_field!(input, $value)),+))? $({$($field: Wire::read(input)?),+})?),+,
                    _ => return Err(DecodeError::Malformed),
                })
            }
        }
    };
}

tagged!(Unit {0 => Establishments, 1 => HeadOffices});
tagged!(FlowMetric {0 => Registrations, 1 => Closures, 2 => NetRegistrations, 3 => MeiOptIns, 4 => MeiExits});
tagged!(RegionLevel {0 => Major, 1 => Intermediate, 2 => Immediate});
tagged!(CnaeLevel {0 => Section, 1 => Division, 2 => Group, 3 => Class, 4 => Subclass});
tagged!(Dir {0 => Asc, 1 => Desc});
tagged!(Grain {0 => Month, 1 => Year});
tagged!(ListOrder {0 => Cnpj, 1 => NewestFirst, 2 => OldestFirst, 3 => CapitalDesc});
tagged!(MetaTopic {0 => DataDate, 1 => Sources, 2 => Methodology, 3 => Help, 4 => Cnaes});
tagged!(CnaeScope {0 => Main, 1 => Any});
tagged!(Tri {0 => Any, 1 => Yes, 2 => No});
tagged!(ContactNeed {0 => Any, 1 => Phone, 2 => Email, 3 => Both});
tagged!(NameField {0 => Any, 1 => Legal, 2 => Trade});
tagged!(CnaeCode {0 => Section(code), 1 => Division(code), 2 => Group(code), 3 => Class(code), 4 => Subclass(code)});
tagged!(PopulationRef {0 => IbgeCensus(year), 1 => IbgeEstimate(year)});
tagged!(Metric {0 => Stock, 1 => Flow(flow), 2 => ActiveChange {pct}, 3 => Per10k(population), 4 => InhabitantsPer(population), 5 => Share});
tagged!(Dim {0 => Municipality, 1 => Uf, 2 => Region(level), 3 => Sector, 4 => Cnae(level), 5 => Porte, 6 => Status, 7 => HeadOffice, 8 => Mei, 9 => Year, 10 => Month});
tagged!(Member {0 => Municipality(id), 1 => Uf(id), 2 => Region(id), 3 => Sector(id)});
tagged!(Op {
    0 => Count {metric}, 1 => List {order, limit}, 2 => Lookup {id}, 3 => Compare {dim, members, metric},
    4 => Rank {dim, metric, dir, k, min_base}, 5 => Breakdown {dim, metric}, 6 => Trend {metric, grain}, 7 => Meta {topic},
});
tagged!(NaturezaCode {0 => Group(prefix), 1 => Code(code)});
tagged!(NearRule {0 => Neighbours(depth), 1 => WithinKm(distance)});
tagged!(PlaceFilter {0 => Any, 1 => Municipalities(ids), 2 => Ufs(ids), 3 => Region(id), 4 => Near {center, rule}});
tagged!(ActivityFilter {0 => Any, 1 => Sectors {ids, cnaes, scope}, 2 => Cnae {codes, scope}});
tagged!(Period {0 => Days(range), 1 => Competences {from, to}});

record!(Exclusions { municipalities, ufs, sectors, sector_cnaes });
record!(Filters {
    place,
    within_ufs,
    place_population,
    population_ref,
    activity,
    status,
    porte,
    mei,
    simples,
    head_office,
    natureza,
    opened,
    status_date,
    capital,
    exclude,
    contact,
} defaults {name_prefix});
record!(TimeSpec { competence, period, anchor });
record!(Output { page, with_methodology, display_order });
record!(CatalogueVersions { municipalities, sectors });
record!(Request { op, unit, scope, time, output, versions });
