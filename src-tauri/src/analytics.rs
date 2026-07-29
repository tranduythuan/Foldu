//! Bang phan tich — chay NGAY SAU khi quet, truoc ca khi quyet dinh don the nao.
//! Muc tieu: nguoi quan ly nhin mot cai la biet thu muc dang co van de gi.

use crate::config::{fallback_group, Settings};
use crate::criteria::DAY_MS;
use crate::scanner::{FileEntry, ScanResult};
use crate::util::now_ms;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub label: String,
    pub count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BigFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    pub total_files: usize,
    pub total_bytes: u64,
    pub scanned_dirs: usize,
    pub max_depth: usize,
    pub avg_depth: f64,
    pub files_at_root: usize,

    pub by_type: Vec<Bucket>,
    pub by_year: Vec<Bucket>,
    pub by_folder: Vec<Bucket>,
    pub top_files: Vec<BigFile>,

    pub cold_count: usize,
    pub cold_bytes: u64,
    pub unnamed_count: usize,
    pub project_folders: usize,
    pub cloud_skipped: usize,
    pub app_folders_protected: usize,
    pub system_protected: usize,

    /// Chi co khi quet nguyen mot phan vung
    pub whole_drive: bool,
    pub drive: Option<crate::safety::DriveInfo>,
    /// Phan dung luong o dia KHONG nam trong pham vi quet
    /// (thu muc he thong, ung dung duoc bao ve, file an...)
    pub out_of_scope_bytes: u64,

    /// 0..100 — cang cao cang gon gang
    pub health: u32,
    pub health_notes: Vec<String>,
}

const UNNAMED_HINTS: &[&str] = &[
    "untitled", "new document", "document1", "khong ten", "tai lieu moi", "copy of",
    "img_", "dsc_", "image_", "unnamed", "download", "new folder",
];

pub fn analyze(scan: &ScanResult, settings: &Settings, roots: &[PathBuf]) -> Analytics {
    let files = &scan.files;
    let now = now_ms();

    let total_files = files.len();
    let total_bytes: u64 = files.iter().map(|f| f.size).sum();

    // ---- Theo nhom loai
    let mut by_type: HashMap<String, (usize, u64)> = HashMap::new();
    for f in files {
        let g = settings
            .groups
            .iter()
            .find(|g| g.exts.iter().any(|e| *e == f.ext))
            .map(|g| g.name.clone())
            .unwrap_or_else(|| fallback_group().to_string());
        let e = by_type.entry(g).or_insert((0, 0));
        e.0 += 1;
        e.1 += f.size;
    }

    // ---- Theo nam sua doi
    let mut by_year: HashMap<String, (usize, u64)> = HashMap::new();
    for f in files {
        let y = crate::util::strftime(f.mtime, "%Y");
        let e = by_year.entry(y).or_insert((0, 0));
        e.0 += 1;
        e.1 += f.size;
    }

    // ---- Theo thu muc CAP 1 duoi goc quet — day moi la thu cho biet
    //      "thu muc nao dang an het o dia", chu khong phai thu muc cha truc tiep.
    let mut by_folder: HashMap<String, (usize, u64)> = HashMap::new();
    for f in files {
        let label = top_level_of(&f.path, &f.root);
        let e = by_folder.entry(label).or_insert((0, 0));
        e.0 += 1;
        e.1 += f.size;
    }

    // ---- File lon nhat
    let mut sorted: Vec<&FileEntry> = files.iter().collect();
    sorted.sort_by(|a, b| b.size.cmp(&a.size));
    let top_files: Vec<BigFile> = sorted
        .iter()
        .take(20)
        .map(|f| BigFile {
            path: f.path.clone(),
            name: f.name.clone(),
            size: f.size,
            mtime: f.mtime,
        })
        .collect();

    // ---- File nguoi
    let cold: Vec<&FileEntry> = files
        .iter()
        .filter(|f| {
            let last = if f.atime > 0 { f.atime.max(f.mtime) } else { f.mtime };
            last > 0 && now - last > 365 * DAY_MS
        })
        .collect();

    // ---- File dat ten vo nghia
    let unnamed_count = files
        .iter()
        .filter(|f| {
            let n = crate::util::norm_key(&f.name);
            UNNAMED_HINTS.iter().any(|h| n.starts_with(h) || n.contains(h))
        })
        .count();

    let max_depth = files.iter().map(|f| f.depth).max().unwrap_or(0);
    let avg_depth = if total_files == 0 {
        0.0
    } else {
        files.iter().map(|f| f.depth as f64).sum::<f64>() / total_files as f64
    };
    let files_at_root = files.iter().filter(|f| f.depth == 0).count();

    // ---- Diem suc khoe
    let mut health: i64 = 100;
    let mut notes: Vec<String> = Vec::new();

    if total_files > 0 {
        let root_ratio = files_at_root as f64 / total_files as f64;
        if root_ratio > 0.5 {
            health -= 25;
            notes.push(crate::i18n::tf("msg.rootAtLarge", &[&format!("{:.0}", root_ratio * 100.0)]));
        } else if root_ratio > 0.25 {
            health -= 12;
            notes.push(crate::i18n::tf("msg.rootSome", &[&format!("{:.0}", root_ratio * 100.0)]));
        }

        let unnamed_ratio = unnamed_count as f64 / total_files as f64;
        if unnamed_ratio > 0.3 {
            health -= 20;
            notes.push(crate::i18n::tf("msg.unnamed", &[&format!("{:.0}", unnamed_ratio * 100.0)]));
        } else if unnamed_ratio > 0.1 {
            health -= 8;
        }

        let cold_ratio = cold.len() as f64 / total_files as f64;
        if cold_ratio > 0.6 {
            health -= 15;
            notes.push(crate::i18n::tf("msg.cold", &[&format!("{:.0}", cold_ratio * 100.0)]));
        }

        if max_depth > 10 {
            health -= 12;
            notes.push(crate::i18n::tf("msg.deep", &[&max_depth.to_string()]));
        }
        if avg_depth < 0.6 && total_files > 200 {
            health -= 10;
            notes.push(crate::i18n::t("msg.flat").into());
        }
    }
    // ---- Thong tin o dia khi quet nguyen mot phan vung
    let drive = if scan.stats.whole_drive {
        roots
            .iter()
            .find(|r| crate::safety::is_drive_root(r))
            .and_then(|r| {
                let letter = crate::util::drive_of(r);
                crate::safety::list_drives()
                    .into_iter()
                    .find(|d| d.letter == letter)
            })
    } else {
        None
    };

    // Phan dung luong da dung nhung KHONG nam trong pham vi quet:
    // thu muc he thong, ung dung duoc bao ve, file an, file dam may...
    let out_of_scope_bytes = drive
        .as_ref()
        .map(|d| d.used.saturating_sub(total_bytes))
        .unwrap_or(0);

    if let Some(d) = &drive {
        let used_pct = if d.total > 0 {
            d.used as f64 / d.total as f64 * 100.0
        } else {
            0.0
        };
        if used_pct > 90.0 {
            notes.push(crate::i18n::tf("msg.driveFull", &[
                &d.letter,
                &format!("{:.0}", used_pct),
                &crate::util::format_bytes(d.free),
            ]));
        }
        if scan.stats.app_folders_protected > 0 {
            notes.push(crate::i18n::tf("msg.appsProtected", &[&scan.stats.app_folders_protected.to_string()]));
        }
    }

    if notes.is_empty() {
        notes.push(crate::i18n::t("msg.tidy").into());
    }

    Analytics {
        total_files,
        total_bytes,
        scanned_dirs: scan.stats.scanned_dirs,
        max_depth,
        avg_depth,
        files_at_root,
        by_type: to_buckets(by_type, 14),
        by_year: to_buckets_sorted_by_label(by_year),
        by_folder: to_buckets(by_folder, 12),
        top_files,
        cold_count: cold.len(),
        cold_bytes: cold.iter().map(|f| f.size).sum(),
        unnamed_count,
        project_folders: scan.stats.project_folders,
        cloud_skipped: scan.stats.cloud_skipped,
        app_folders_protected: scan.stats.app_folders_protected,
        system_protected: scan.stats.system_protected,
        whole_drive: scan.stats.whole_drive,
        drive,
        out_of_scope_bytes,
        health: health.clamp(0, 100) as u32,
        health_notes: notes,
    }
}

/// Thu muc cap 1 chua file, tinh tu goc quet.
/// "D:\Phim\2026\a.mp4" voi goc "D:\" -> "Phim". File nam ngay goc -> "(ngay ở gốc)".
fn top_level_of(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => {
            let mut it = rel.components();
            match (it.next(), it.next()) {
                // Con thanh phan phia sau -> thanh phan dau la mot thu muc
                (Some(first), Some(_)) => first.as_os_str().to_string_lossy().to_string(),
                // Chi con mot thanh phan -> chinh la ten file, tuc file nam ngay goc
                _ => crate::i18n::t("lbl.atRoot").to_string(),
            }
        }
        Err(_) => path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| crate::i18n::t("lbl.misc").to_string()),
    }
}

fn to_buckets(m: HashMap<String, (usize, u64)>, limit: usize) -> Vec<Bucket> {
    let mut v: Vec<Bucket> = m
        .into_iter()
        .map(|(label, (count, bytes))| Bucket {
            label,
            count,
            bytes,
        })
        .collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    if v.len() > limit {
        let rest: Vec<Bucket> = v.split_off(limit);
        v.push(Bucket {
            label: crate::i18n::t("lbl.otherBucket").into(),
            count: rest.iter().map(|b| b.count).sum(),
            bytes: rest.iter().map(|b| b.bytes).sum(),
        });
    }
    v
}

fn to_buckets_sorted_by_label(m: HashMap<String, (usize, u64)>) -> Vec<Bucket> {
    let mut v: Vec<Bucket> = m
        .into_iter()
        .map(|(label, (count, bytes))| Bucket {
            label,
            count,
            bytes,
        })
        .collect();
    v.sort_by(|a, b| a.label.cmp(&b.label));
    v
}
