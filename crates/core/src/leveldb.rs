//! 브라우저 프로필의 LevelDB 를 읽기만 하는 작은 구현 (닫힌 탭 그룹을 읽는 데 쓴다).
//!
//! Chrome · Edge · Whale 은 동기화 데이터를 프로필의 `Sync Data/LevelDB` 에 둔다. 브라우저가 켜져 있어도
//! 파일을 바꾸지 않고 읽기만 한다. 로그(.log)와 표(.ldb · .sst)를 모두 읽어, 키마다 가장 나중에 쓴 값을 고른다.
//! 형식: https://github.com/google/leveldb/blob/main/doc/log_format.md , table_format.md

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LevelDbError {
    #[error("{}", crate::tr!("LevelDB 파일이 깨져 있습니다", "The LevelDB file is corrupt"))]
    Corrupt,
    #[error("{}", crate::tr!("지원하지 않는 압축 방식입니다 ({})", "Unsupported compression ({})", .0))]
    UnsupportedCompression(u8),
}

/// 한 번 쓰인 값: 쓴 순번(sequence)이 클수록 나중 것이다. 지운 기록이면 value 가 None.
struct Write {
    sequence: u64,
    value: Option<Vec<u8>>,
}

/// 폴더의 모든 키와 (지워지지 않은) 마지막 값. `prefix` 로 시작하는 키만 모은다.
pub fn read_directory(directory: &Path, prefix: &[u8]) -> std::io::Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let mut latest: BTreeMap<Vec<u8>, Write> = BTreeMap::new();
    let mut record = |key: &[u8], sequence: u64, value: Option<&[u8]>| {
        if !key.starts_with(prefix) {
            return;
        }
        let newer = latest.get(key).is_none_or(|existing| sequence >= existing.sequence);
        if newer {
            latest.insert(key.to_vec(), Write { sequence, value: value.map(<[u8]>::to_vec) });
        }
    };
    let mut entries: Vec<_> = fs::read_dir(directory)?.filter_map(|entry| entry.ok()).map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
        // 브라우저가 쓰는 중이면 끝부분이 덜 써져 있을 수 있다. 읽을 수 있는 데까지만 쓴다.
        let Ok(data) = fs::read(&path) else {
            continue;
        };
        match extension {
            "log" => read_log(&data, &mut record),
            "ldb" | "sst" => {
                let _ = read_table(&data, &mut record);
            }
            _ => {}
        }
    }
    Ok(latest.into_iter().filter_map(|(key, write)| write.value.map(|value| (key, value))).collect())
}

// MARK: - 로그 파일

const BLOCK_SIZE: usize = 32 * 1024;

/// 로그 파일의 쓰기 묶음(write batch)들을 읽는다.
fn read_log(data: &[u8], record: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) {
    let mut batch: Vec<u8> = Vec::new();
    let mut offset = 0;
    while offset + 7 <= data.len() {
        let block_left = BLOCK_SIZE - offset % BLOCK_SIZE;
        if block_left < 7 {
            // 블록 끝의 남는 자리는 0 으로 채워져 있다.
            offset += block_left;
            continue;
        }
        let length = u16::from_le_bytes([data[offset + 4], data[offset + 5]]) as usize;
        let kind = data[offset + 6];
        let start = offset + 7;
        let Some(fragment) = data.get(start..start + length) else {
            break;
        };
        offset = start + length;
        match kind {
            // 통째로 · 첫 조각 · 중간 · 마지막 조각
            1 => {
                read_write_batch(fragment, record);
                batch.clear();
            }
            2 => {
                batch.clear();
                batch.extend_from_slice(fragment);
            }
            3 => batch.extend_from_slice(fragment),
            4 => {
                batch.extend_from_slice(fragment);
                read_write_batch(&batch, record);
                batch.clear();
            }
            // 0: 미리 잡아 둔 빈 자리
            _ => {}
        }
    }
}

fn read_write_batch(data: &[u8], record: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) {
    let Some(header) = data.get(..12) else {
        return;
    };
    let first = u64::from_le_bytes(header[..8].try_into().unwrap_or_default());
    let count = u32::from_le_bytes(header[8..12].try_into().unwrap_or_default());
    let mut reader = Reader::new(&data[12..]);
    // 묶음 안의 기록은 첫 순번부터 하나씩 늘어난다.
    for sequence in (first..).take(count as usize) {
        let Some(tag) = reader.byte() else {
            return;
        };
        let Some(key) = reader.length_prefixed() else {
            return;
        };
        match tag {
            1 => {
                let Some(value) = reader.length_prefixed() else {
                    return;
                };
                record(key, sequence, Some(value));
            }
            0 => record(key, sequence, None),
            _ => return,
        }
    }
}

// MARK: - 표 파일

const TABLE_MAGIC: u64 = 0xdb47_7524_8b80_fb57;

fn read_table(data: &[u8], record: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> Result<(), LevelDbError> {
    if data.len() < 48 {
        return Err(LevelDbError::Corrupt);
    }
    let footer = &data[data.len() - 48..];
    if u64::from_le_bytes(footer[40..48].try_into().map_err(|_| LevelDbError::Corrupt)?) != TABLE_MAGIC {
        return Err(LevelDbError::Corrupt);
    }
    let mut reader = Reader::new(footer);
    let _metaindex = reader.block_handle().ok_or(LevelDbError::Corrupt)?;
    let index = reader.block_handle().ok_or(LevelDbError::Corrupt)?;
    let index_block = read_block(data, index)?;
    for (_, handle) in block_entries(&index_block)? {
        let Some(handle) = Reader::new(&handle).block_handle() else {
            continue;
        };
        let Ok(block) = read_block(data, handle) else {
            continue;
        };
        let Ok(entries) = block_entries(&block) else {
            continue;
        };
        for (internal_key, value) in entries {
            // 내부 키 = 사용자 키 + 8바이트(순번 << 8 | 종류)
            let Some(split) = internal_key.len().checked_sub(8) else {
                continue;
            };
            let trailer = u64::from_le_bytes(internal_key[split..].try_into().unwrap_or_default());
            let (sequence, kind) = (trailer >> 8, trailer & 0xff);
            let key = &internal_key[..split];
            match kind {
                1 => record(key, sequence, Some(&value)),
                0 => record(key, sequence, None),
                _ => {}
            }
        }
    }
    Ok(())
}

/// 블록을 읽어 압축을 푼다. 블록 뒤에는 압축 방식 1바이트와 체크섬 4바이트가 붙어 있다.
fn read_block(data: &[u8], (offset, size): (u64, u64)) -> Result<Vec<u8>, LevelDbError> {
    let start = usize::try_from(offset).map_err(|_| LevelDbError::Corrupt)?;
    let size = usize::try_from(size).map_err(|_| LevelDbError::Corrupt)?;
    let contents = data.get(start..start.checked_add(size).ok_or(LevelDbError::Corrupt)?).ok_or(LevelDbError::Corrupt)?;
    let compression = *data.get(start + size).ok_or(LevelDbError::Corrupt)?;
    match compression {
        0 => Ok(contents.to_vec()),
        1 => snappy_decompress(contents),
        other => Err(LevelDbError::UnsupportedCompression(other)),
    }
}

/// 블록의 (키, 값)들. 키는 앞 항목과 겹치는 앞부분을 빼고 적혀 있다.
/// 블록 안의 (키, 값)
type BlockEntry = (Vec<u8>, Vec<u8>);

fn block_entries(block: &[u8]) -> Result<Vec<BlockEntry>, LevelDbError> {
    let restarts = u32::from_le_bytes(block.get(block.len().saturating_sub(4)..).and_then(|bytes| bytes.try_into().ok()).ok_or(LevelDbError::Corrupt)?) as usize;
    let end = block.len().checked_sub(4 + restarts * 4).ok_or(LevelDbError::Corrupt)?;
    let mut reader = Reader::new(&block[..end]);
    let mut key: Vec<u8> = Vec::new();
    let mut entries = Vec::new();
    while !reader.is_empty() {
        let shared = reader.varint().ok_or(LevelDbError::Corrupt)? as usize;
        let unshared = reader.varint().ok_or(LevelDbError::Corrupt)? as usize;
        let value_length = reader.varint().ok_or(LevelDbError::Corrupt)? as usize;
        if shared > key.len() {
            return Err(LevelDbError::Corrupt);
        }
        key.truncate(shared);
        key.extend_from_slice(reader.bytes(unshared).ok_or(LevelDbError::Corrupt)?);
        let value = reader.bytes(value_length).ok_or(LevelDbError::Corrupt)?;
        entries.push((key.clone(), value.to_vec()));
    }
    Ok(entries)
}

// MARK: - Snappy

/// Snappy 로 압축한 블록을 푼다 (https://github.com/google/snappy/blob/main/format_description.txt).
pub fn snappy_decompress(input: &[u8]) -> Result<Vec<u8>, LevelDbError> {
    let mut reader = Reader::new(input);
    let length = reader.varint().ok_or(LevelDbError::Corrupt)? as usize;
    // 깨진 길이로 메모리를 크게 잡지 않는다 (압축률은 많아야 수십 배).
    if length > input.len().saturating_mul(64).max(1 << 20) {
        return Err(LevelDbError::Corrupt);
    }
    let mut output: Vec<u8> = Vec::with_capacity(length);
    while !reader.is_empty() {
        let tag = reader.byte().ok_or(LevelDbError::Corrupt)?;
        match tag & 3 {
            0 => {
                let mut literal = (tag >> 2) as usize;
                if literal >= 60 {
                    let bytes = reader.bytes(literal - 59).ok_or(LevelDbError::Corrupt)?;
                    literal = bytes.iter().rev().fold(0usize, |value, byte| (value << 8) | *byte as usize);
                }
                output.extend_from_slice(reader.bytes(literal + 1).ok_or(LevelDbError::Corrupt)?);
            }
            kind => {
                let (count, distance) = match kind {
                    1 => (((tag >> 2) & 7) as usize + 4, ((tag as usize >> 5) << 8) | reader.byte().ok_or(LevelDbError::Corrupt)? as usize),
                    2 => {
                        let bytes = reader.bytes(2).ok_or(LevelDbError::Corrupt)?;
                        ((tag >> 2) as usize + 1, u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
                    }
                    _ => {
                        let bytes = reader.bytes(4).ok_or(LevelDbError::Corrupt)?;
                        ((tag >> 2) as usize + 1, u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
                    }
                };
                if distance == 0 || distance > output.len() {
                    return Err(LevelDbError::Corrupt);
                }
                let start = output.len() - distance;
                // 겹쳐 복사할 수 있어(거리 < 길이) 한 바이트씩 옮긴다.
                for index in 0..count {
                    let byte = output[start + index];
                    output.push(byte);
                }
            }
        }
        if output.len() > length {
            return Err(LevelDbError::Corrupt);
        }
    }
    if output.len() != length {
        return Err(LevelDbError::Corrupt);
    }
    Ok(output)
}

// MARK: - 읽기 도우미

pub(crate) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub(crate) fn byte(&mut self) -> Option<u8> {
        let (first, rest) = self.data.split_first()?;
        self.data = rest;
        Some(*first)
    }

    pub(crate) fn bytes(&mut self, count: usize) -> Option<&'a [u8]> {
        if count > self.data.len() {
            return None;
        }
        let (taken, rest) = self.data.split_at(count);
        self.data = rest;
        Some(taken)
    }

    pub(crate) fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn length_prefixed(&mut self) -> Option<&'a [u8]> {
        let length = usize::try_from(self.varint()?).ok()?;
        self.bytes(length)
    }

    fn block_handle(&mut self) -> Option<(u64, u64)> {
        Some((self.varint()?, self.varint()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snappy() {
        // 리터럴 "abc" + 거리 3 · 길이 6 복사(겹침) → "abcabcabc"
        let compressed = [9u8, 2 << 2, b'a', b'b', b'c', ((6 - 4) << 2) | 1, 3];
        assert_eq!(snappy_decompress(&compressed).unwrap(), b"abcabcabc");
        assert_eq!(snappy_decompress(&[3, 0]), Err(LevelDbError::Corrupt), "모자란 입력");
        // 길이 4, 복사(1바이트 거리 9) — 아직 아무것도 풀지 않았는데 9 만큼 앞을 가리킨다
        assert_eq!(snappy_decompress(&[4, 1, 9]), Err(LevelDbError::Corrupt), "출력보다 먼 거리");
    }

    #[test]
    fn log_records() {
        // 쓰기 묶음: 순번 7, 2개 (a=1 넣기, b 지우기)
        let mut batch = 7u64.to_le_bytes().to_vec();
        batch.extend_from_slice(&2u32.to_le_bytes());
        batch.extend_from_slice(&[1, 1, b'a', 1, b'1', 0, 1, b'b']);
        let mut log = vec![0, 0, 0, 0];
        log.extend_from_slice(&(batch.len() as u16).to_le_bytes());
        log.push(1);
        log.extend_from_slice(&batch);
        let mut seen = Vec::new();
        read_log(&log, &mut |key, sequence, value| seen.push((key.to_vec(), sequence, value.map(<[u8]>::to_vec))));
        assert_eq!(seen, [(b"a".to_vec(), 7, Some(b"1".to_vec())), (b"b".to_vec(), 8, None)]);
    }
}
