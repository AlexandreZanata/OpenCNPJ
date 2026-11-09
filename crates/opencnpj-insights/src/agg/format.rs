use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Write,
    path::Path,
    sync::OnceLock,
};

use anyhow::{Context, Result, ensure};
use memmap2::{Mmap, MmapOptions};
use opencnpj_core::varint::{self, Reader};
use xxhash_rust::xxh3::xxh3_64;

use super::{BLOCK_CELLS, Cell, Key, Manifest, Section, Table};

/// Tables that can have an activity rollup, indexed by `Table as usize - 1`.
const TABLES: usize = Table::ALL.len();

const MAGIC: &[u8; 8] = b"OCAGG\0\0\0";
const FOOTER: &[u8; 8] = b"OCAGEND!";
const VERSION: [u8; 4] = [0, 0, 1, 0];
const HEADER: usize = 32;
const ENTRY: usize = 48;
const MAX_MANIFEST: usize = 64 * 1024;

#[derive(Clone, Copy)]
struct Block {
    section: Section,
    compressed: bool,
    count: u16,
    raw_len: u32,
    stored_len: u32,
    offset: u64,
    first: u64,
    last: u64,
    checksum: u64,
}

impl Block {
    fn write(self, out: &mut Vec<u8>) {
        out.extend([self.section.tag(), u8::from(self.compressed)]);
        out.extend(self.count.to_le_bytes());
        out.extend(self.raw_len.to_le_bytes());
        out.extend(self.stored_len.to_le_bytes());
        out.extend([0; 4]);
        for value in [self.offset, self.first, self.last, self.checksum] {
            out.extend(value.to_le_bytes());
        }
    }

    fn read(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let section = Section::from_tag(r.u8()?).context("unknown aggregate section")?;
        let codec = r.u8()?;
        ensure!(codec <= 1, "unknown aggregate block codec");
        let count = u16::from_le_bytes(r.bytes(2)?.try_into()?);
        let raw_len = r.u32_le()?;
        let stored_len = r.u32_le()?;
        ensure!(r.u32_le()? == 0, "nonzero aggregate block reserved bytes");
        ensure!(count > 0 && usize::from(count) <= BLOCK_CELLS, "invalid aggregate block cell count");
        let size = u32::from(count);
        ensure!((1 + size + size.div_ceil(8)..=1 + 18 * size).contains(&raw_len), "invalid aggregate raw block size");
        ensure!(stored_len > 0 && if codec == 0 { stored_len == raw_len } else { stored_len < raw_len }, "invalid aggregate stored size");
        let offset = r.u64_le()?;
        let first = r.u64_le()?;
        let last = r.u64_le()?;
        let checksum = r.u64_le()?;
        ensure!(first <= last && (first == last) == (count == 1), "invalid aggregate block key range");
        Ok(Self { section, compressed: codec == 1, count, raw_len, stored_len, offset, first, last, checksum })
    }
}

pub fn write(mut out: impl Write, manifest: &Manifest, sections: &BTreeMap<Section, Vec<Cell>>) -> Result<()> {
    validate_manifest(manifest)?;
    let metadata = serde_json::to_vec(manifest)?;
    ensure!(metadata.len() <= MAX_MANIFEST, "aggregate manifest is too large");
    let mut data = vec![0; HEADER];
    data.extend(&metadata);
    let mut blocks = Vec::new();
    let mut raw = Vec::new();
    let mut compressor = zstd::bulk::Compressor::new(3)?;
    for (&section, cells) in sections {
        ensure!(manifest.tables.contains(&section.table()), "section missing from aggregate manifest: {section:?}");
        ensure!(cells.windows(2).all(|pair| pair[0].key < pair[1].key), "aggregate cells must be sorted and unique");
        for chunk in cells.chunks(BLOCK_CELLS) {
            raw.clear();
            let largest = chunk.iter().map(|cell| cell.count).max().expect("nonempty block");
            let width = (u64::BITS - largest.leading_zeros()) as u8;
            raw.push(width);
            let mut previous = 0;
            for cell in chunk {
                ensure!(cell.count > 0, "aggregate cells must have positive counts");
                varint::put(&mut raw, cell.key - previous);
                previous = cell.key;
            }
            pack_counts(&mut raw, chunk, width);
            let compressed = compressor.compress(&raw)?;
            let use_compressed = compressed.len() < raw.len();
            let stored = if use_compressed { &compressed } else { &raw };
            blocks.push(Block {
                section,
                compressed: use_compressed,
                count: chunk.len() as u16,
                raw_len: raw.len() as u32,
                stored_len: stored.len() as u32,
                offset: data.len() as u64,
                first: chunk[0].key,
                last: previous,
                checksum: xxh3_64(&raw),
            });
            data.extend(stored);
        }
    }
    let directory = data.len() as u64;
    let count = u32::try_from(blocks.len())?;
    for block in blocks {
        block.write(&mut data);
    }
    data[..8].copy_from_slice(MAGIC);
    data[8..12].copy_from_slice(&VERSION);
    data[12..16].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
    data[16..20].copy_from_slice(&count.to_le_bytes());
    data[20..28].copy_from_slice(&directory.to_le_bytes());
    data.extend(xxh3_64(&data).to_le_bytes());
    data.extend(FOOTER);
    out.write_all(&data)?;
    out.flush()?;
    Ok(())
}

pub struct Aggregate {
    data: Mmap,
    manifest: Manifest,
    sections: BTreeMap<Section, Vec<Block>>,
    /// Cells summed over every CNAE, built on first use (see [`Aggregate::activity_rollup`]).
    rollups: [OnceLock<Result<Vec<Cell>, String>>; TABLES],
}

impl Aggregate {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open aggregate {}", path.display()))?;
        // The artifact is published by atomic rename and must be kept immutable
        // while it is mapped, like the source snapshot.
        let data = unsafe { MmapOptions::new().map(&file)? };
        let end = data.len().checked_sub(16).context("truncated aggregate footer")?;
        ensure!(end >= HEADER && &data[end + 8..] == FOOTER, "invalid aggregate footer");
        ensure!(xxh3_64(&data[..end]) == u64::from_le_bytes(data[end..end + 8].try_into()?), "aggregate checksum mismatch");
        let mut r = Reader::new(&data[..HEADER]);
        ensure!(r.bytes(8)? == MAGIC && r.bytes(4)? == VERSION, "invalid aggregate magic or version");
        let metadata_len = r.u32_le()? as usize;
        let blocks = r.u32_le()? as usize;
        let directory = usize::try_from(r.u64_le()?)?;
        ensure!(r.u32_le()? == 0 && metadata_len <= MAX_MANIFEST, "invalid aggregate header");
        let metadata_end = HEADER + metadata_len;
        ensure!(
            directory >= metadata_end && directory <= end && (end - directory).is_multiple_of(ENTRY) && (end - directory) / ENTRY == blocks,
            "invalid aggregate directory bounds"
        );
        let manifest: Manifest = serde_json::from_slice(&data[HEADER..metadata_end])?;
        validate_manifest(&manifest)?;
        let mut sections = BTreeMap::<Section, Vec<Block>>::new();
        let mut position = metadata_end;
        let mut previous: Option<Block> = None;
        for bytes in data[directory..end].as_chunks::<ENTRY>().0 {
            let block = Block::read(bytes)?;
            ensure!(manifest.tables.contains(&block.section.table()), "undeclared aggregate section");
            ensure!(
                block.offset == position as u64 && (block.stored_len as usize) <= directory - position,
                "aggregate block outside its payload"
            );
            position += block.stored_len as usize;
            if let Some(prev) = previous {
                ensure!(
                    prev.section < block.section || (prev.section == block.section && prev.last < block.first),
                    "unordered aggregate blocks"
                );
            }
            sections.entry(block.section).or_default().push(block);
            previous = Some(block);
        }
        ensure!(position == directory, "unused bytes before aggregate directory");
        Ok(Self { data, manifest, sections, rollups: Default::default() })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn cells(&self, section: Section) -> impl Iterator<Item = Result<Cell>> + '_ {
        self.decode_cells(self.sections.get(&section).map_or(&[], Vec::as_slice))
    }

    /// The cells of `table` with the CNAE set to zero and equal keys summed, in key order.
    /// A question over every activity reads this instead of the whole CNAE-major table;
    /// counts are additive, so any reader that ignores the CNAE gets the same totals.
    pub fn activity_rollup(&self, table: Table) -> Result<&[Cell]> {
        let built = self.rollups[table as usize - 1].get_or_init(|| {
            let mut sums: HashMap<u64, u64> = HashMap::new();
            for cell in self.cells(Section::Cells(table)) {
                let cell = cell.map_err(|error| error.to_string())?;
                let key = Key { cnae: 0, ..Key::unpack(cell.key) }.pack().ok_or("invalid aggregate key")?;
                let sum = sums.entry(key).or_default();
                *sum = sum.checked_add(cell.count).ok_or("aggregate rollup overflow")?;
            }
            let mut cells: Vec<Cell> = sums.into_iter().map(|(key, count)| Cell { key, count }).collect();
            cells.sort_unstable_by_key(|cell| cell.key);
            Ok(cells)
        });
        built.as_deref().map_err(|error| anyhow::anyhow!("{error}"))
    }

    /// Build the activity rollups of every table in the artifact (a service calls this at
    /// start-up so the first question over all activities is as fast as the next).
    pub fn warm(&self) -> Result<()> {
        for table in self.manifest.tables.clone() {
            self.activity_rollup(table)?;
        }
        Ok(())
    }

    pub fn cells_for_cnae(&self, table: Table, cnae: u32) -> Result<impl Iterator<Item = Result<Cell>> + '_> {
        let index = self.get(Section::CnaeOffset(table), u64::from(cnae))?;
        let blocks = if index == 0 {
            &[]
        } else {
            let blocks = self.sections.get(&Section::Cells(table)).context("missing CNAE indexed section")?;
            let start = usize::try_from(index - 1)?;
            let tail = blocks.get(start..).context("CNAE block index out of bounds")?;
            let first = tail.first().context("empty CNAE block range")?;
            ensure!(Key::unpack(first.first).cnae <= cnae && Key::unpack(first.last).cnae >= cnae, "CNAE index points to another code");
            ensure!(start == 0 || Key::unpack(blocks[start - 1].last).cnae < cnae, "CNAE index skips a block");
            let count = tail.partition_point(|block| Key::unpack(block.first).cnae <= cnae);
            &tail[..count]
        };
        Ok(self.decode_cells(blocks).filter(move |cell| match cell {
            Ok(cell) => Key::unpack(cell.key).cnae == cnae,
            Err(_) => true,
        }))
    }

    fn decode_cells<'a>(&'a self, blocks: &'a [Block]) -> impl Iterator<Item = Result<Cell>> + 'a {
        blocks.iter().flat_map(|block| match self.decode(*block) {
            Ok(cells) => cells.into_iter().map(Ok).collect::<Vec<_>>(),
            Err(error) => vec![Err(error)],
        })
    }

    pub fn get(&self, section: Section, key: u64) -> Result<u64> {
        let Some(blocks) = self.sections.get(&section) else { return Ok(0) };
        let index = blocks.partition_point(|block| block.first <= key);
        if index == 0 || key > blocks[index - 1].last {
            return Ok(0);
        }
        let cells = self.decode(blocks[index - 1])?;
        Ok(cells.binary_search_by_key(&key, |cell| cell.key).map_or(0, |i| cells[i].count))
    }

    fn decode(&self, block: Block) -> Result<Vec<Cell>> {
        let offset = block.offset as usize;
        let stored = &self.data[offset..offset + block.stored_len as usize];
        let uncompressed;
        let raw = if block.compressed {
            uncompressed = zstd::bulk::decompress(stored, block.raw_len as usize)?;
            uncompressed.as_slice()
        } else {
            stored
        };
        ensure!(raw.len() == block.raw_len as usize && xxh3_64(raw) == block.checksum, "aggregate block checksum or length mismatch");
        let mut r = Reader::new(raw);
        let width = r.u8()?;
        ensure!((1..=64).contains(&width), "invalid aggregate count bit width");
        let mut cells = Vec::with_capacity(usize::from(block.count));
        let mut previous = 0u64;
        for i in 0..block.count {
            let delta = r.varint()?;
            ensure!(i == 0 || delta > 0, "invalid aggregate key delta");
            let key = previous.checked_add(delta).context("aggregate key overflow")?;
            cells.push(Cell { key, count: 0 });
            previous = key;
        }
        let counts_len = (usize::from(block.count) * usize::from(width)).div_ceil(8);
        unpack_counts(r.bytes(counts_len)?, &mut cells, width)?;
        ensure!(r.is_empty() && cells[0].key == block.first && previous == block.last, "aggregate block key or count mismatch");
        Ok(cells)
    }
}

fn pack_counts(out: &mut Vec<u8>, cells: &[Cell], width: u8) {
    let mut pending = 0u128;
    let mut bits = 0;
    for cell in cells {
        pending |= u128::from(cell.count) << bits;
        bits += width;
        while bits >= 8 {
            out.push(pending as u8);
            pending >>= 8;
            bits -= 8;
        }
    }
    if bits > 0 {
        out.push(pending as u8);
    }
}

fn unpack_counts(bytes: &[u8], cells: &mut [Cell], width: u8) -> Result<()> {
    let mut bytes = bytes.iter();
    let mut pending = 0u128;
    let mut bits = 0;
    let mask = (1u128 << width) - 1;
    for cell in cells {
        while bits < width {
            pending |= u128::from(*bytes.next().context("truncated aggregate counts")?) << bits;
            bits += 8;
        }
        cell.count = (pending & mask) as u64;
        ensure!(cell.count > 0, "invalid zero aggregate count");
        pending >>= width;
        bits -= width;
    }
    ensure!(pending == 0, "nonzero aggregate count padding");
    Ok(())
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    ensure!(!manifest.tables.is_empty() && manifest.tables.windows(2).all(|pair| pair[0] < pair[1]), "invalid aggregate table list");
    for hash in [&manifest.snapshot_sha256, &manifest.municipality_sha256] {
        ensure!(hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "invalid aggregate provenance SHA-256");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use opencnpj_query::ir::Competence;

    use super::*;
    use crate::agg::Table;

    #[test]
    fn activity_rollup_sums_every_cnae_per_place_and_detail() {
        use opencnpj_query::ir::{MunicipalityId, UfId};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollup.ocagg");
        let manifest = Manifest {
            competence: Competence(321),
            snapshot_sha256: "1".repeat(64),
            municipality_sha256: "2".repeat(64),
            tables: vec![Table::T3],
            establishments: 10000,
            missing_registration_date: 0,
            missing_closure_date: 0,
        };
        let key =
            |cnae, municipality, detail| Key { cnae, municipality: MunicipalityId(municipality), uf: UfId(12), detail }.pack().unwrap();
        // Several CNAE blocks share places and months; the rollup must add them up.
        let mut cells = Vec::new();
        for cnae in [1091102, 4721102, 4771701, 6201501] {
            for municipality in 0..1500 {
                cells.push(Cell { key: key(cnae, municipality, 300 + u32::from(municipality % 3)), count: u64::from(cnae % 7 + 1) });
            }
        }
        cells.sort_by_key(|cell| cell.key);
        let section = Section::Cells(Table::T3);
        let sections = BTreeMap::from([(section, cells.clone()), (Section::CnaeOffset(Table::T3), vec![Cell { key: 0, count: 1 }])]);
        let mut data = Vec::new();
        write(&mut data, &manifest, &sections).unwrap();
        fs::write(&path, &data).unwrap();
        let agg = Aggregate::open(&path).unwrap();
        agg.warm().unwrap();
        let rollup = agg.activity_rollup(Table::T3).unwrap();
        let mut expected = BTreeMap::<u64, u64>::new();
        for cell in &cells {
            *expected.entry(Key { cnae: 0, ..Key::unpack(cell.key) }.pack().unwrap()).or_default() += cell.count;
        }
        assert_eq!(rollup, expected.into_iter().map(|(key, count)| Cell { key, count }).collect::<Vec<_>>());
        assert_eq!(rollup.len(), 1500);
        assert_eq!(rollup.iter().map(|c| c.count).sum::<u64>(), cells.iter().map(|c| c.count).sum::<u64>());
    }

    #[test]
    fn maps_cells_across_blocks_and_rejects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.ocagg");
        let manifest = Manifest {
            competence: Competence(321),
            snapshot_sha256: "1".repeat(64),
            municipality_sha256: "2".repeat(64),
            tables: vec![Table::T1],
            establishments: 10000,
            missing_registration_date: 0,
            missing_closure_date: 0,
        };
        let section = Section::Cells(Table::T1);
        let cells = (0..5000).map(|key| Cell { key: key * 2, count: key + 1 }).collect::<Vec<_>>();
        let sections = BTreeMap::from([(section, cells.clone()), (Section::CnaeOffset(Table::T1), vec![Cell { key: 0, count: 1 }])]);
        let mut data = Vec::new();
        write(&mut data, &manifest, &sections).unwrap();
        fs::write(&path, &data).unwrap();
        let agg = Aggregate::open(&path).unwrap();
        assert_eq!(agg.manifest(), &manifest);
        assert_eq!(agg.cells(section).collect::<Result<Vec<_>>>().unwrap(), cells);
        assert_eq!(agg.cells_for_cnae(Table::T1, 0).unwrap().collect::<Result<Vec<_>>>().unwrap(), cells);
        assert_eq!(agg.get(section, 8192).unwrap(), 4097);
        assert_eq!(agg.get(section, 8191).unwrap(), 0);
        drop(agg);
        fs::write(&path, &data[..data.len() - 1]).unwrap();
        assert!(Aggregate::open(&path).is_err());
        data[HEADER] ^= 1;
        fs::write(&path, &data).unwrap();
        assert!(Aggregate::open(&path).is_err());
        data[HEADER] ^= 1;
        data[20..28].copy_from_slice(&0u64.to_le_bytes());
        let end = data.len() - 16;
        let checksum = xxh3_64(&data[..end]);
        data[end..end + 8].copy_from_slice(&checksum.to_le_bytes());
        fs::write(&path, &data).unwrap();
        assert!(Aggregate::open(&path).is_err());
    }
}
