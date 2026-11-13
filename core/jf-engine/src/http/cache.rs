//! The byte cache behind an HTTP stream: fixed-size blocks in memory, spilled to a capped
//! temporary file when memory runs over its budget.
//!
//! Downloads fill blocks from their start, so a block is always a valid prefix (`len` bytes).
//! Memory holds the read-ahead window; blocks outside it are written to the spill file (when one
//! is configured and has room) or dropped, farthest from the reader first.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Block size in bytes.
pub(crate) const BLOCK: u64 = 64 * 1024;
/// Largest spill file per stream.
pub(crate) const SPILL_CAP: u64 = 256 << 20;
/// Longest run [`Cache::contiguous_from`] measures.
const CONTIGUOUS_SCAN_MAX: u64 = 64 << 20;

#[derive(Debug)]
enum Data {
    Memory(Vec<u8>),
    /// In the spill file at `slot * BLOCK`.
    Disk(u64),
}

#[derive(Debug)]
struct Block {
    data: Data,
    /// Valid bytes from the block's start.
    len: usize,
}

#[derive(Debug)]
struct Spill {
    dir: PathBuf,
    file: Option<File>,
    next_slot: u64,
    max_slots: u64,
    /// The file failed once; stop using it.
    broken: bool,
}

#[derive(Debug)]
pub(crate) struct Cache {
    blocks: BTreeMap<u64, Block>,
    memory_bytes: u64,
    memory_cap: u64,
    spill: Option<Spill>,
}

impl Cache {
    /// A cache keeping at most `memory_cap` bytes in memory, spilling to a temporary file in
    /// `spill_dir` (up to `spill_cap` bytes) when given.
    pub(crate) fn new(memory_cap: u64, spill_dir: Option<PathBuf>, spill_cap: u64) -> Self {
        Self {
            blocks: BTreeMap::new(),
            memory_bytes: 0,
            memory_cap: memory_cap.max(BLOCK * 4),
            spill: spill_dir.map(|dir| Spill {
                dir,
                file: None,
                next_slot: 0,
                max_slots: spill_cap / BLOCK,
                broken: false,
            }),
        }
    }

    pub(crate) fn set_memory_cap(&mut self, cap: u64) {
        self.memory_cap = cap.max(BLOCK * 4);
    }

    fn is_complete(block: &Block, index: u64, total: Option<u64>) -> bool {
        block.len as u64 == BLOCK || total.is_some_and(|t| index * BLOCK + block.len as u64 >= t)
    }

    /// Copies cached bytes at `pos` into `buf`; 0 when the byte at `pos` is not cached.
    pub(crate) fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let (index, offset) = (pos / BLOCK, (pos % BLOCK) as usize);
        let Some(block) = self.blocks.get(&index) else {
            return Ok(0);
        };
        if offset >= block.len {
            return Ok(0);
        }
        let n = buf.len().min(block.len - offset);
        match &block.data {
            Data::Memory(bytes) => buf[..n].copy_from_slice(&bytes[offset..offset + n]),
            Data::Disk(slot) => {
                let slot = *slot;
                let file = self
                    .spill
                    .as_mut()
                    .and_then(|s| s.file.as_mut())
                    .ok_or_else(|| io::Error::other("spill file missing"))?;
                file.seek(SeekFrom::Start(slot * BLOCK + offset as u64))?;
                file.read_exact(&mut buf[..n])?;
            }
        }
        Ok(n)
    }

    /// Bytes cached without a gap from `pos` (up to 64 MiB are measured).
    pub(crate) fn contiguous_from(&self, pos: u64) -> u64 {
        let mut index = pos / BLOCK;
        let mut end = pos;
        while end - pos < CONTIGUOUS_SCAN_MAX {
            let Some(block) = self.blocks.get(&index) else {
                break;
            };
            let block_end = index * BLOCK + block.len as u64;
            if block_end <= end {
                break;
            }
            end = block_end;
            if (block.len as u64) < BLOCK {
                break;
            }
            index += 1;
        }
        end - pos
    }

    /// The first byte at or after the start of `pos`'s block that is not cached, if it lies
    /// before `limit`. Downloads start there, so blocks fill from their start.
    pub(crate) fn missing_from(&self, pos: u64, limit: u64, total: Option<u64>) -> Option<u64> {
        let mut index = pos / BLOCK;
        loop {
            let start = index * BLOCK;
            if start >= limit || total.is_some_and(|t| start >= t) {
                return None;
            }
            match self.blocks.get(&index) {
                None => return Some(start),
                Some(block) if !Self::is_complete(block, index, total) => {
                    let at = start + block.len as u64;
                    return (at < limit).then_some(at);
                }
                Some(_) => index += 1,
            }
        }
    }

    /// Stores downloaded bytes that start at `pos`. Returns how many of them the cache took
    /// (bytes already cached count as taken); fewer than `data.len()` only when `pos` does not
    /// continue a block's valid prefix.
    pub(crate) fn write_at(&mut self, mut pos: u64, mut data: &[u8]) -> usize {
        let mut taken = 0;
        while !data.is_empty() {
            let (index, offset) = (pos / BLOCK, (pos % BLOCK) as usize);
            let room = BLOCK as usize - offset;
            let n = room.min(data.len());
            let block = self.blocks.entry(index);
            match block {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    if offset != 0 {
                        break;
                    }
                    let mut bytes = Vec::with_capacity(BLOCK as usize);
                    bytes.extend_from_slice(&data[..n]);
                    slot.insert(Block {
                        data: Data::Memory(bytes),
                        len: n,
                    });
                    self.memory_bytes += BLOCK;
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    let block = slot.get_mut();
                    if offset > block.len {
                        break;
                    }
                    let fresh = (offset + n).saturating_sub(block.len);
                    if fresh > 0 {
                        match &mut block.data {
                            Data::Memory(bytes) => {
                                bytes.extend_from_slice(&data[n - fresh..n]);
                                block.len += fresh;
                            }
                            // Spilled blocks are complete.
                            Data::Disk(_) => {}
                        }
                    }
                }
            }
            pos += n as u64;
            data = &data[n..];
            taken += n;
        }
        taken
    }

    /// Brings memory back under its budget, keeping `[keep_from, keep_to)`: blocks farthest
    /// from `keep_from` go to the spill file when they are complete and it has room, otherwise
    /// they are dropped.
    pub(crate) fn evict(&mut self, keep_from: u64, keep_to: u64, total: Option<u64>) {
        if self.memory_bytes <= self.memory_cap {
            return;
        }
        let (first_kept, last_kept) = (keep_from / BLOCK, keep_to.div_ceil(BLOCK));
        let mut candidates: Vec<u64> = self
            .blocks
            .iter()
            .filter(|(index, block)| {
                matches!(block.data, Data::Memory(_)) && (**index < first_kept || **index >= last_kept)
            })
            .map(|(index, _)| *index)
            .collect();
        candidates.sort_by_key(|index| std::cmp::Reverse(index.abs_diff(first_kept)));
        for index in candidates {
            if self.memory_bytes <= self.memory_cap {
                break;
            }
            let Some(block) = self.blocks.remove(&index) else {
                continue;
            };
            self.memory_bytes -= BLOCK;
            let complete = Self::is_complete(&block, index, total);
            if let (true, Data::Memory(bytes)) = (complete, &block.data)
                && let Some(slot) = self.spill_block(bytes)
            {
                self.blocks.insert(
                    index,
                    Block {
                        data: Data::Disk(slot),
                        len: block.len,
                    },
                );
            }
        }
    }

    /// Writes a block to the spill file; `None` when there is none, it is full or failed.
    fn spill_block(&mut self, bytes: &[u8]) -> Option<u64> {
        let spill = self.spill.as_mut()?;
        if spill.broken || spill.next_slot >= spill.max_slots {
            return None;
        }
        if spill.file.is_none() {
            match tempfile::tempfile_in(&spill.dir) {
                Ok(file) => spill.file = Some(file),
                Err(err) => {
                    tracing::warn!(
                        "cannot create a stream cache file in {}: {err}",
                        spill.dir.display()
                    );
                    spill.broken = true;
                    return None;
                }
            }
        }
        let slot = spill.next_slot;
        let file = spill.file.as_mut()?;
        let written = file
            .seek(SeekFrom::Start(slot * BLOCK))
            .and_then(|_| file.write_all(bytes));
        match written {
            Ok(()) => {
                spill.next_slot += 1;
                Some(slot)
            }
            Err(err) => {
                tracing::warn!("stream cache file: {err}; keeping the stream in memory only");
                spill.broken = true;
                None
            }
        }
    }

    /// Bytes held in memory (whole blocks).
    #[cfg(test)]
    pub(crate) fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    /// Bytes held in the spill file.
    pub(crate) fn spilled_bytes(&self) -> u64 {
        self.spill.as_ref().map_or(0, |s| s.next_slot * BLOCK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(from: u64, len: usize) -> Vec<u8> {
        (from..from + len as u64).map(|i| (i % 251) as u8).collect()
    }

    fn read_all(cache: &mut Cache, pos: u64, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        let mut done = 0;
        while done < len {
            let n = cache.read_at(pos + done as u64, &mut out[done..]).unwrap();
            assert!(n > 0, "missing byte at {}", pos + done as u64);
            done += n;
        }
        out
    }

    #[test]
    fn sequential_writes_are_read_back_across_blocks() {
        let mut cache = Cache::new(64 << 20, None, 0);
        let data = pattern(0, 200_000);
        assert_eq!(cache.write_at(0, &data[..100_000]), 100_000);
        assert_eq!(cache.write_at(100_000, &data[100_000..]), 100_000);
        assert_eq!(read_all(&mut cache, 0, 200_000), data);
        assert_eq!(cache.contiguous_from(1_000), 199_000);
        assert_eq!(cache.missing_from(0, u64::MAX, None), Some(200_000));
        assert_eq!(cache.missing_from(0, u64::MAX, Some(200_000)), None);
    }

    #[test]
    fn a_gap_is_reported_and_a_download_must_start_at_a_block_start() {
        let mut cache = Cache::new(64 << 20, None, 0);
        // Not at a block start and not continuing one: refused.
        assert_eq!(cache.write_at(BLOCK + 10, &[1, 2, 3]), 0);
        cache.write_at(2 * BLOCK, &pattern(2 * BLOCK, 100));
        assert_eq!(cache.missing_from(0, u64::MAX, None), Some(0));
        assert_eq!(
            cache.missing_from(2 * BLOCK + 5, u64::MAX, None),
            Some(2 * BLOCK + 100)
        );
        assert_eq!(cache.missing_from(2 * BLOCK, 2 * BLOCK + 50, None), None);
        let mut buf = [0u8; 8];
        assert_eq!(cache.read_at(10, &mut buf).unwrap(), 0);
        assert_eq!(cache.contiguous_from(2 * BLOCK + 40), 60);
    }

    #[test]
    fn overlapping_writes_keep_the_first_copy() {
        let mut cache = Cache::new(64 << 20, None, 0);
        let data = pattern(0, 1_000);
        cache.write_at(0, &data[..600]);
        assert_eq!(cache.write_at(0, &data), 1_000);
        assert_eq!(read_all(&mut cache, 0, 1_000), data);
    }

    #[test]
    fn eviction_spills_complete_blocks_and_reads_them_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = Cache::new(BLOCK * 4, Some(dir.path().to_path_buf()), SPILL_CAP);
        let data = pattern(0, (BLOCK * 10) as usize);
        cache.write_at(0, &data);
        cache.evict(8 * BLOCK, 10 * BLOCK, Some(10 * BLOCK));
        assert!(cache.memory_bytes() <= BLOCK * 4, "{}", cache.memory_bytes());
        assert!(cache.spilled_bytes() >= BLOCK * 6);
        assert_eq!(read_all(&mut cache, 0, data.len()), data);
    }

    #[test]
    fn without_a_spill_file_eviction_drops_the_farthest_blocks() {
        let mut cache = Cache::new(BLOCK * 4, None, 0);
        cache.write_at(0, &pattern(0, (BLOCK * 10) as usize));
        cache.evict(6 * BLOCK, 8 * BLOCK, None);
        assert!(cache.memory_bytes() <= BLOCK * 4);
        // The kept window and its nearest neighbours survive; the far start is gone.
        assert_eq!(cache.missing_from(0, u64::MAX, None), Some(0));
        assert!(cache.contiguous_from(6 * BLOCK) >= 2 * BLOCK);
    }
}
