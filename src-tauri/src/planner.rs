//! Lap KE HOACH — buoc nay TUYET DOI khong ghi gi vao o dia.
//!
//! Nho tach bach nay, ban Xem truoc va luc Ap dung dung chung mot ma nguon:
//! cai nguoi dung nhin thay chinh xac la cai se xay ra.

use crate::clustering::{cluster_projects, ClusterInput};
use crate::config::{DupAction, Mode, Profile, Settings, SIDECAR_GROUPS};
use crate::criteria::{self, Ctx};
use crate::dedup::{self, DupReport};
use crate::media::{self, MediaInfo};
use crate::scanner::FileEntry;
use crate::util::{ext_of, now_ms, split_name};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn dup_folder() -> &'static str {
    crate::i18n::t("seg.duplicates")
}

/// Ảnh gần giống được để riêng, KHÔNG lẫn với thư mục trùng lặp tuyệt đối.
/// Hai loại này có độ chắc chắn khác hẳn nhau nên không được trộn.
pub fn near_folder() -> &'static str {
    crate::i18n::t("seg.nearDupes")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OpAction {
    Move,
    Copy,
    Hardlink,
    /// Ban thua cua mot nhom trung lap -> dua vao thu muc cach ly
    Quarantine,
    /// Ban thua -> dua vao Thung rac Windows
    Recycle,
    /// Giu nguyen tai cho (khong can di chuyen)
    Keep,
    /// Bo qua (bi khoa, duong dan qua dai...)
    Skip,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOp {
    pub id: u32,
    pub src: PathBuf,
    pub dest: PathBuf,
    /// Ten file cuoi cung (co the da doi de tranh ghi de)
    pub final_name: String,
    pub size: u64,
    pub action: OpAction,
    /// Tieu chi nao quyet dinh dich den nay
    pub reason: String,
    pub renamed: bool,
    pub is_dir: bool,
    pub selected: bool,
    /// Anh gan giong, khac voi trung lap tuyet doi: do chac chan thap hon han
    pub near: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    pub total: usize,
    pub moves: usize,
    pub renames: usize,
    pub duplicates: usize,
    pub keeps: usize,
    pub skips: usize,
    pub new_folders: usize,
    pub total_bytes: u64,
    pub dup_wasted: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub ops: Vec<PlanOp>,
    pub summary: PlanSummary,
    pub folders: Vec<String>,
    pub dup_report: DupReport,
    pub near_report: crate::phash::NearReport,
    pub warnings: Vec<String>,
    pub mode: Mode,
    pub roots: Vec<PathBuf>,
    /// Sau khi chuyen file, co don cac vo thu muc rong con lai khong
    pub clean_empty_dirs: bool,
    pub elapsed_ms: u64,
}

// ------------------------------------------------------- Chuan bi ngu canh

fn probe_worthy(f: &FileEntry) -> bool {
    const MEDIA_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "heif", "tif", "tiff", "avif", "jfif",
        "cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2",
    ];
    MEDIA_EXTS.contains(&f.ext.as_str())
}

fn build_context<'a>(
    files: &[FileEntry],
    profile: &Profile,
    settings: &'a Settings,
    mut on_progress: impl FnMut(&str, usize, usize),
) -> Ctx<'a> {
    let layers = &profile.layers;

    // Đổi tên dùng token "ngày chụp" cũng cần đọc EXIF, không thì token đó lặng
    // lẽ dùng ngày sửa — đúng cái nó sinh ra để tránh.
    let rename_needs_taken = profile.rename.enabled
        && profile.rename.parts.iter().any(|p| p.kind == "taken");

    // --- Metadata media (doc lazy, chay song song)
    let media: HashMap<u32, MediaInfo> = if criteria::needs_media(layers) || rename_needs_taken {
        let need_all = layers
            .iter()
            .any(|l| criteria::parse_layer(l).0 == "REAL_TYPE");
        let targets: Vec<&FileEntry> = files
            .iter()
            .filter(|f| !f.is_dir && (need_all || probe_worthy(f)))
            .collect();
        on_progress(crate::i18n::t("prog.readMedia"), 0, targets.len());
        let out = targets
            .par_iter()
            .map(|f| (f.id, media::probe(&f.path)))
            .collect();
        on_progress(crate::i18n::t("prog.readMedia"), targets.len(), targets.len());
        out
    } else {
        HashMap::new()
    };

    // --- Nguon tai ve (Zone.Identifier)
    let sources: HashMap<u32, String> = if criteria::needs_download_source(layers) {
        files
            .par_iter()
            .filter(|f| !f.is_dir)
            .filter_map(|f| media::download_source(&f.path).map(|d| (f.id, d)))
            .collect()
    } else {
        HashMap::new()
    };

    // --- Cum du an
    let clusters = if criteria::needs_clustering(layers) {
        on_progress(crate::i18n::t("prog.clustering"), 0, files.len());
        let inputs: Vec<ClusterInput> = files
            .iter()
            .map(|f| ClusterInput {
                id: f.id,
                name: &f.name,
            })
            .collect();
        cluster_projects(
            &inputs,
            &settings.noise_words,
            profile.clustering.granularity,
            profile.clustering.max_tokens,
            profile.safety.max_new_folders,
        )
    } else {
        HashMap::new()
    };

    // --- Nhom phien ban
    let versions = if criteria::needs_versions(layers) {
        criteria::build_version_groups(files)
    } else {
        HashMap::new()
    };

    let mut sizes: Vec<u64> = files.iter().filter(|f| !f.is_dir).map(|f| f.size).collect();
    let size_p = criteria::percentiles(&mut sizes);

    Ctx {
        settings,
        now_ms: now_ms(),
        clusters,
        versions,
        media,
        sources,
        size_p,
        number_prefix: profile.number_prefix,
    }
}

// -------------------------------------------------------------- Bo file di kem

/// Gom cac file di kem nhau (RAW+JPG, mp4+srt, psd+preview) de chung KHONG bi tach roi.
/// Tra ve: id file -> id file "chu" quyet dinh thu muc dich.
fn build_sidecar_bundles(files: &[FileEntry]) -> HashMap<u32, u32> {
    let mut by_key: HashMap<(PathBuf, String), Vec<&FileEntry>> = HashMap::new();
    for f in files {
        if f.is_dir || f.ext.is_empty() {
            continue;
        }
        let (stem, _) = split_name(&f.name);
        by_key
            .entry((f.parent.clone(), stem.to_lowercase()))
            .or_default()
            .push(f);
    }

    let mut out = HashMap::new();
    for (_k, members) in by_key {
        if members.len() < 2 {
            continue;
        }
        // Tat ca duoi file phai cung nam trong MOT nhom sidecar
        let exts: Vec<&str> = members.iter().map(|m| m.ext.as_str()).collect();
        let in_group = SIDECAR_GROUPS
            .iter()
            .any(|g| exts.iter().all(|e| g.contains(e)));
        if !in_group {
            continue;
        }
        // File "chu" = file lon nhat (thuong la ban goc/RAW/video)
        let host = members.iter().max_by_key(|m| m.size).unwrap();
        for m in &members {
            if m.id != host.id {
                out.insert(m.id, host.id);
            }
        }
    }
    out
}

// ------------------------------------------------------------------ Lap ke hoach

/// Tinh phan NANG: loc trung lap (BLAKE3 toan file) + anh gan giong (giai ma +
/// dHash tung anh). Chi phu thuoc TAP FILE va cai dat trung lap, KHONG phu thuoc
/// cach sap xep. Nho vay lop lenh o tren cache lai ket qua nay va chi tinh lai
/// khi doi scan hoac doi cai dat trung lap — doi tieu chi sap xep khong tinh lai.
pub fn compute_dup_near(
    files: &[FileEntry],
    profile: &Profile,
    mut on_progress: impl FnMut(&str, usize, usize),
) -> (DupReport, crate::phash::NearReport) {
    // --- Trung lap
    let dup_report: DupReport = if profile.duplicates.enabled {
        on_progress(crate::i18n::t("prog.findDupes"), 0, files.len());
        let r = dedup::find_duplicates(files, profile.duplicates.strategy, |a, b| {
            on_progress(crate::i18n::t("prog.hashing"), a, b)
        });
        on_progress(crate::i18n::t("prog.findDupes"), files.len(), files.len());
        r
    } else {
        DupReport::default()
    };

    // Bo cac file da la ban thua tuyet doi ra khoi phep so anh gan giong, de mot
    // file khong bi bao hai lan.
    let mut dup_extra: HashSet<u32> = HashSet::new();
    for g in &dup_report.groups {
        for e in &g.extras {
            dup_extra.insert(e.id);
        }
    }

    // --- Ảnh gần giống nhau.
    let near_report = if profile.duplicates.enabled && profile.duplicates.near_images {
        on_progress(crate::i18n::t("prog.nearDupes"), 0, files.len());
        let r = crate::phash::find_near_duplicate_images(
            files,
            profile.duplicates.near_threshold,
            &dup_extra,
            |a, b| on_progress(crate::i18n::t("prog.nearDupes"), a, b),
        );
        on_progress(crate::i18n::t("prog.nearDupes"), files.len(), files.len());
        r
    } else {
        crate::phash::NearReport::default()
    };

    (dup_report, near_report)
}

pub fn build_plan(
    files: &[FileEntry],
    profile: &Profile,
    settings: &Settings,
    roots: &[PathBuf],
    mut on_progress: impl FnMut(&str, usize, usize),
) -> Plan {
    let (dup_report, near_report) = compute_dup_near(files, profile, &mut on_progress);
    build_plan_with_reports(files, profile, settings, roots, on_progress, dup_report, near_report)
}

/// Nhu `build_plan` nhung nhan san bao cao trung lap/anh gan giong da tinh truoc
/// (tu cache), nen doi tieu chi sap xep chi ton chi phi sap xep, khong bam lai file.
pub fn build_plan_with_reports(
    files: &[FileEntry],
    profile: &Profile,
    settings: &Settings,
    roots: &[PathBuf],
    mut on_progress: impl FnMut(&str, usize, usize),
    dup_report: DupReport,
    near_report: crate::phash::NearReport,
) -> Plan {
    let start = std::time::Instant::now();
    let mut warnings: Vec<String> = Vec::new();

    let ctx = build_context(files, profile, settings, &mut on_progress);

    let mut dup_extra: HashSet<u32> = HashSet::new();
    for g in &dup_report.groups {
        for e in &g.extras {
            dup_extra.insert(e.id);
        }
    }

    let mut near_extra: HashSet<u32> = HashSet::new();
    for g in &near_report.groups {
        for e in &g.extras {
            near_extra.insert(e.id);
        }
    }

    // --- Bo file di kem
    let bundles: HashMap<u32, u32> = if profile.safety.keep_sidecar_together {
        build_sidecar_bundles(files)
    } else {
        HashMap::new()
    };

    let by_id: HashMap<u32, &FileEntry> = files.iter().map(|f| (f.id, f)).collect();

    // --- Sinh thu muc dich cho tung file
    let reason_label = if profile.layers.is_empty() {
        crate::i18n::t("msg.noLayers").to_string()
    } else {
        profile
            .layers
            .iter()
            .map(|l| criteria::parse_layer(l).0)
            .collect::<Vec<_>>()
            .join(" › ")
    };

    let base_for = |f: &FileEntry| -> PathBuf {
        match &profile.destination {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => f.root.clone(),
        }
    };

    // Đổi tên "để yên chỗ" thì mỗi file ở nguyên thư mục cha, chỉ tên đổi.
    // Đây là cách an toàn và dễ hiểu nhất, nên là mặc định khi đổi tên.
    let rename_in_place = profile.rename.enabled && profile.rename.in_place;

    let mut dir_of: HashMap<u32, PathBuf> = HashMap::new();
    for f in files {
        let dir = if rename_in_place {
            f.parent.clone()
        } else {
            let mut dir = base_for(f);
            for layer in &profile.layers {
                for s in criteria::segments(layer, f, &ctx) {
                    dir.push(s);
                }
            }
            dir
        };
        dir_of.insert(f.id, dir);
    }

    // --- Đánh số thứ tự cho việc đổi tên: theo TỪNG thư mục đích, sắp theo tên gốc.
    //     Cùng một tập file thì luôn cho ra cùng thứ tự, nên preview khớp lúc chạy.
    let counter_of: HashMap<u32, u32> = if profile.rename.enabled {
        let mut by_dir: HashMap<String, Vec<&FileEntry>> = HashMap::new();
        for f in files {
            if f.is_dir || dup_extra.contains(&f.id) || near_extra.contains(&f.id) {
                continue;
            }
            let key = dir_of
                .get(&f.id)
                .map(|d| d.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            by_dir.entry(key).or_default().push(f);
        }
        let mut map = HashMap::new();
        for (_dir, mut members) in by_dir {
            members.sort_by(|a, b| {
                crate::rename::counter_order_key(&a.name)
                    .cmp(&crate::rename::counter_order_key(&b.name))
            });
            for (i, m) in members.iter().enumerate() {
                map.insert(m.id, i as u32);
            }
        }
        map
    } else {
        HashMap::new()
    };

    // Dựng tên mới cho một file theo mẫu người dùng. Trả về tên gốc nếu không đổi.
    let renamed_name = |f: &FileEntry, dir: &Path| -> String {
        if !profile.rename.enabled || f.is_dir {
            return f.name.clone();
        }
        let folder = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let taken_ms = ctx.media.get(&f.id).and_then(|m| m.taken_ms).unwrap_or(0);
        crate::rename::render_name(
            &profile.rename.parts,
            &profile.rename.transforms,
            &crate::rename::RenameCtx {
                original: &f.name,
                mtime: f.mtime,
                ctime: f.ctime,
                taken: taken_ms,
                folder: &folder,
                counter: *counter_of.get(&f.id).unwrap_or(&0),
            },
        )
    };

    // File di kem phai theo thu muc cua file "chu"
    for (child, host) in &bundles {
        if let Some(hd) = dir_of.get(host).cloned() {
            dir_of.insert(*child, hd);
        }
    }

    // --- Sinh thao tac + xu ly dung do ten
    let mode = profile.mode;
    let mut taken: HashSet<String> = HashSet::new();
    let mut ops: Vec<PlanOp> = Vec::with_capacity(files.len());
    let mut folders: HashSet<String> = HashSet::new();

    for f in files {
        // Ban thua trong nhom trung lap
        // Ảnh gần giống: LUÔN dồn vào một thư mục riêng để người dùng tự xem lại.
        // Không bao giờ đưa vào Thùng rác dù người dùng đặt thế cho trùng lặp,
        // vì đây chỉ là "trông giống", máy có thể nhầm còn file thì mất thật.
        if near_extra.contains(&f.id) {
            let dir = base_for(f).join(near_folder());
            let (name, _) = unique_name(&dir, &f.name, &mut taken);
            folders.insert(dir.to_string_lossy().to_string());
            let dest = dir.join(&name);
            ops.push(PlanOp {
                id: f.id,
                src: f.path.clone(),
                final_name: name,
                dest,
                size: f.size,
                action: OpAction::Quarantine,
                reason: crate::i18n::t("msg.nearReason").into(),
                renamed: false,
                is_dir: f.is_dir,
                selected: true,
                near: true,
            });
            continue;
        }

        if dup_extra.contains(&f.id) && profile.duplicates.action != DupAction::Report {
            let (action, dir) = match profile.duplicates.action {
                DupAction::Recycle => (OpAction::Recycle, PathBuf::new()),
                _ => (OpAction::Quarantine, base_for(f).join(dup_folder())),
            };
            let dest = if action == OpAction::Recycle {
                PathBuf::new()
            } else {
                let (name, renamed) = unique_name(&dir, &f.name, &mut taken);
                let _ = renamed;
                folders.insert(dir.to_string_lossy().to_string());
                dir.join(name)
            };
            ops.push(PlanOp {
                id: f.id,
                src: f.path.clone(),
                final_name: dest
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.name.clone()),
                dest,
                size: f.size,
                action,
                reason: crate::i18n::t("msg.dupReason").into(),
                renamed: false,
                is_dir: f.is_dir,
                selected: true,
                near: false,
            });
            continue;
        }

        let dir = dir_of.get(&f.id).cloned().unwrap_or_else(|| base_for(f));
        let want_name = renamed_name(f, &dir);

        // Cùng thư mục VÀ tên không đổi -> thật sự không phải làm gì.
        // Nếu tên đổi thì dù cùng thư mục vẫn là một thao tác đổi tên.
        if crate::util::same_path(&dir, &f.parent) && want_name == f.name {
            ops.push(PlanOp {
                id: f.id,
                src: f.path.clone(),
                dest: f.path.clone(),
                final_name: f.name.clone(),
                size: f.size,
                action: OpAction::Keep,
                reason: crate::i18n::t("msg.alreadyThere").into(),
                renamed: false,
                is_dir: f.is_dir,
                selected: false,
                near: false,
            });
            taken.insert(f.path.to_string_lossy().to_lowercase());
            continue;
        }

        let (name, collided) = unique_name(&dir, &want_name, &mut taken);
        // "renamed" = tên cuối khác tên gốc, bất kể do mẫu hay do chống đè
        let renamed = name != f.name;
        let _ = collided;
        let dest = dir.join(&name);
        folders.insert(dir.to_string_lossy().to_string());

        let action = match mode {
            Mode::Move => OpAction::Move,
            Mode::Copy => OpAction::Copy,
            Mode::Hardlink => OpAction::Hardlink,
            Mode::ReportOnly => OpAction::Keep,
        };

        ops.push(PlanOp {
            id: f.id,
            src: f.path.clone(),
            dest,
            final_name: name,
            size: f.size,
            action,
            reason: if rename_in_place {
                crate::i18n::t("msg.renameReason").into()
            } else if bundles.contains_key(&f.id) {
                crate::i18n::tf("msg.withSidecar", &[&reason_label])
            } else {
                reason_label.clone()
            },
            renamed,
            is_dir: f.is_dir,
            selected: true,
            near: false,
        });
    }

    // --- Canh bao
    if folders.len() > profile.safety.max_new_folders {
        warnings.push(crate::i18n::tf("msg.tooManyFolders", &[
            &folders.len().to_string(),
            &profile.safety.max_new_folders.to_string(),
        ]));
    }
    let other_count = ops
        .iter()
        .filter(|o| o.dest.to_string_lossy().contains(crate::clustering::other()))
        .count();
    if criteria::needs_clustering(&profile.layers) && other_count * 2 > ops.len() && !ops.is_empty()
    {
        warnings.push(crate::i18n::tf("msg.tooMuchOther", &[
            &other_count.to_string(),
            &ops.len().to_string(),
            crate::clustering::other(),
        ]));
    }

    let summary = PlanSummary {
        total: ops.len(),
        moves: ops
            .iter()
            .filter(|o| matches!(o.action, OpAction::Move | OpAction::Copy | OpAction::Hardlink))
            .count(),
        renames: ops.iter().filter(|o| o.renamed).count(),
        duplicates: ops
            .iter()
            .filter(|o| matches!(o.action, OpAction::Quarantine | OpAction::Recycle))
            .count(),
        keeps: ops.iter().filter(|o| o.action == OpAction::Keep).count(),
        skips: ops.iter().filter(|o| o.action == OpAction::Skip).count(),
        new_folders: folders.len(),
        total_bytes: ops.iter().filter(|o| o.selected).map(|o| o.size).sum(),
        dup_wasted: dup_report.total_wasted,
    };

    let _ = by_id;
    let mut folder_list: Vec<String> = folders.into_iter().collect();
    folder_list.sort();

    Plan {
        ops,
        summary,
        folders: folder_list,
        dup_report,
        near_report,
        warnings,
        mode,
        roots: roots.to_vec(),
        clean_empty_dirs: profile.safety.clean_empty_dirs,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

// ------------------------------------------------------------ Xu ly dung do ten

/// Sinh ten khong dung do. KHONG dung de quy (thu muc co hang nghin file trung ten
/// se lam tran stack) — dung vong lap.
fn unique_name(dir: &Path, name: &str, taken: &mut HashSet<String>) -> (String, bool) {
    let first = dir.join(name);
    let key = first.to_string_lossy().to_lowercase();
    if !taken.contains(&key) && !first.exists() {
        taken.insert(key);
        return (name.to_string(), false);
    }

    let (stem, ext) = split_name(name);
    for i in 1..100_000u32 {
        let candidate = format!("{} ({}){}", stem, i, ext);
        let p = dir.join(&candidate);
        let k = p.to_string_lossy().to_lowercase();
        if !taken.contains(&k) && !p.exists() {
            taken.insert(k);
            return (candidate, true);
        }
    }
    // Truong hop cuc doan — dung dau thoi gian
    let fallback = format!("{} ({}){}", stem, now_ms(), ext);
    taken.insert(dir.join(&fallback).to_string_lossy().to_lowercase());
    (fallback, true)
}

#[allow(dead_code)]
fn ext_unused(s: &str) -> String {
    ext_of(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_name_increments() {
        let dir = std::env::temp_dir().join("foldu-test-unique-nonexistent");
        let mut taken = HashSet::new();
        let (a, r1) = unique_name(&dir, "a.txt", &mut taken);
        let (b, r2) = unique_name(&dir, "a.txt", &mut taken);
        let (c, _) = unique_name(&dir, "a.txt", &mut taken);
        assert_eq!(a, "a.txt");
        assert!(!r1);
        assert_eq!(b, "a (1).txt");
        assert!(r2);
        assert_eq!(c, "a (2).txt");
    }
}
