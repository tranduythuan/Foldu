//! Phat hien trung lap bang loc 3 tang — nhanh hon nhieu so voi bam MD5 toan bo.
//!
//!   Tang 1: gom theo kich thuoc     -> loai ~95% file, khong doc byte nao
//!   Tang 2: bam nhanh 8KB dau+cuoi  -> loai tiep ~99% phan con lai
//!   Tang 3: bam BLAKE3 toan file    -> chay song song, chi voi file con nghi ngo
//!
//! BLAKE3 nhanh hon MD5 khoang 5-10 lan va khong bi va cham co chu dich.

use crate::config::DupStrategy;
use crate::scanner::FileEntry;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const QUICK_CHUNK: usize = 8 * 1024;
const FULL_BUF: usize = 256 * 1024;

fn quick_hash(path: &Path, size: u64) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut h = blake3::Hasher::new();
    h.update(&size.to_le_bytes());

    let mut buf = vec![0u8; QUICK_CHUNK];
    let n = f.read(&mut buf).ok()?;
    h.update(&buf[..n]);

    if size > (QUICK_CHUNK * 2) as u64 {
        f.seek(SeekFrom::End(-(QUICK_CHUNK as i64))).ok()?;
        let n2 = f.read(&mut buf).ok()?;
        h.update(&buf[..n2]);
    }
    Some(h.finalize().to_hex().to_string())
}

pub fn full_hash(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; FULL_BUF];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return None,
        };
    }
    Some(h.finalize().to_hex().to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupMember {
    pub id: u32,
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub hash: String,
    pub size: u64,
    /// File duoc GIU lai
    pub keeper: DupMember,
    /// Cac ban thua
    pub extras: Vec<DupMember>,
    /// Dung luong lang phi = size * so ban thua
    pub wasted: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DupReport {
    pub groups: Vec<DupGroup>,
    pub total_groups: usize,
    pub total_extras: usize,
    pub total_wasted: u64,
    pub hashed_files: usize,
    pub elapsed_ms: u64,
}

/// Diem "sach" cua ten file — cang thap cang sach
fn dirtiness(name: &str) -> i64 {
    let n = crate::util::norm_key(name);
    let mut score = n.chars().count() as i64;
    for pat in [
        "- copy", "copy", "ban sao", "(1)", "(2)", "(3)", " - sao chep", "duplicate", "moi",
    ] {
        if n.contains(pat) {
            score += 200;
        }
    }
    score
}

fn choose_keeper(members: &mut Vec<DupMember>, strategy: DupStrategy) -> DupMember {
    match strategy {
        DupStrategy::KeepOldest => {
            members.sort_by_key(|m| m.mtime);
        }
        DupStrategy::KeepNewest => {
            members.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        }
        DupStrategy::KeepShortestPath => {
            members.sort_by_key(|m| (m.path.as_os_str().len(), m.path.to_string_lossy().to_string()));
        }
        DupStrategy::KeepCleanestName => {
            members.sort_by_key(|m| {
                let name = m
                    .path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                (dirtiness(&name), m.path.as_os_str().len())
            });
        }
    }
    members.remove(0)
}

/// Chay loc 3 tang. `on_progress(da_bam, tong_can_bam)`
/// Chi duoc goi giua cac giai doan, khong bao gio tu ben trong luong rayon —
/// nen khong can rang buoc `Send`.
pub fn find_duplicates<F>(
    files: &[FileEntry],
    strategy: DupStrategy,
    mut on_progress: F,
) -> DupReport
where
    F: FnMut(usize, usize),
{
    let start = std::time::Instant::now();

    // ---- Tang 1: gom theo kich thuoc
    let mut by_size: HashMap<u64, Vec<&FileEntry>> = HashMap::new();
    for f in files {
        if f.is_dir || f.size == 0 {
            continue;
        }
        by_size.entry(f.size).or_default().push(f);
    }
    let stage1: Vec<&FileEntry> = by_size
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .flat_map(|(_, v)| v)
        .collect();

    if stage1.is_empty() {
        return DupReport {
            elapsed_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
    }

    // ---- Tang 2: bam nhanh 8KB dau + cuoi
    let quick: Vec<(&FileEntry, String)> = stage1
        .par_iter()
        .filter_map(|f| quick_hash(&f.path, f.size).map(|h| (*f, h)))
        .collect();

    let mut by_quick: HashMap<(u64, String), Vec<&FileEntry>> = HashMap::new();
    for (f, h) in quick {
        by_quick.entry((f.size, h)).or_default().push(f);
    }
    let stage2: Vec<&FileEntry> = by_quick
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .flat_map(|(_, v)| v)
        .collect();

    if stage2.is_empty() {
        return DupReport {
            elapsed_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
    }

    // ---- Tang 3: bam BLAKE3 toan file (song song)
    let total = stage2.len();
    on_progress(0, total);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let full: Vec<(&FileEntry, String)> = stage2
        .par_iter()
        .filter_map(|f| {
            let r = full_hash(&f.path).map(|h| (*f, h));
            done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            r
        })
        .collect();
    on_progress(total, total);

    let mut by_hash: HashMap<String, Vec<DupMember>> = HashMap::new();
    for (f, h) in &full {
        by_hash.entry(h.clone()).or_default().push(DupMember {
            id: f.id,
            path: f.path.clone(),
            size: f.size,
            mtime: f.mtime,
        });
    }

    let mut groups = Vec::new();
    let mut total_extras = 0usize;
    let mut total_wasted = 0u64;
    for (hash, mut members) in by_hash {
        if members.len() < 2 {
            continue;
        }
        let size = members[0].size;
        let keeper = choose_keeper(&mut members, strategy);
        let wasted = size * members.len() as u64;
        total_extras += members.len();
        total_wasted += wasted;
        groups.push(DupGroup {
            hash,
            size,
            keeper,
            extras: members,
            wasted,
        });
    }
    groups.sort_by(|a, b| b.wasted.cmp(&a.wasted));

    DupReport {
        total_groups: groups.len(),
        groups,
        total_extras,
        total_wasted,
        hashed_files: total,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirtiness_prefers_clean() {
        assert!(dirtiness("bao cao.pdf") < dirtiness("bao cao - Copy.pdf"));
        assert!(dirtiness("a.pdf") < dirtiness("a (1).pdf"));
    }
}
