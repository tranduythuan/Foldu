//! Thuc thi ke hoach — day la lop DUY NHAT duoc phep ghi vao o dia.
//!
//! Nguyen tac an toan:
//!   * Cung o dia  -> rename (tuc thoi, nguyen tu)
//!   * Khac o dia  -> copy -> KIEM TRA HASH ban dich -> chi khi khop moi xoa nguon
//!   * Khong bao gio ghi de: ten dung do da duoc planner giai quyet tu truoc
//!   * Khong bao gio xoa vinh vien: moi thao tac xoa deu vao Thung rac Windows
//!   * Giu nguyen dau thoi gian cua file sau khi di chuyen

use crate::dedup::full_hash;
use crate::journal::{self, Journal};
use crate::planner::{OpAction, Plan, PlanOp};
use crate::util::{drive_of, same_path};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub session: String,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    pub bytes: u64,
    /// Số vỏ thư mục rỗng đã được dọn sau khi chuyển file
    pub removed_dirs: usize,
    pub errors: Vec<ExecError>,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecError {
    pub path: String,
    pub msg: String,
}

pub struct ExecProgress {
    pub index: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub current: String,
}

// ────────────────────────────────────────────── Dọn vỏ thư mục rỗng còn lại

/// Sau khi lôi hết file ra khỏi các thư mục con, chỗ cũ chỉ còn lại vỏ rỗng.
/// Không dọn thì người dùng vẫn phải tự xoá tay một đống thư mục trống.
///
/// Duyệt hậu thứ tự (từ trong ra ngoài) để thư mục cha rỗng theo cũng được dọn.
/// KHÔNG BAO GIỜ xoá chính thư mục gốc người dùng chọn, không xoá mục hệ thống,
/// không đi theo lối tắt. Mỗi thư mục bị dọn đều ghi vào nhật ký để hoàn tác dựng lại.
fn sweep_empty_dirs(root: &Path, jr: &mut Journal, removed: &mut Vec<PathBuf>) {
    fn walk(dir: &Path, root: &Path, jr: &mut Journal, removed: &mut Vec<PathBuf>, depth: usize) {
        if depth > 64 {
            return;
        }
        let entries: Vec<fs::DirEntry> = match fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };
        for e in &entries {
            let md = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !md.is_dir() || crate::scanner::is_reparse_point(&md) {
                continue;
            }
            walk(&e.path(), root, jr, removed, depth + 1);
        }

        // Không bao giờ đụng vào chính thư mục người dùng đã chọn
        if crate::util::same_path(dir, root) {
            return;
        }
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if crate::config::DRIVE_ROOT_PROTECTED.contains(&name.as_str()) {
            return;
        }
        let empty = fs::read_dir(dir)
            .map(|mut i| i.next().is_none())
            .unwrap_or(false);
        if empty && fs::remove_dir(dir).is_ok() {
            removed.push(dir.to_path_buf());
            let _ = jr.record_removed_dir(dir);
        }
    }
    walk(root, root, jr, removed, 0);
}

// ------------------------------------------------------------------ Ho tro

/// Giu nguyen dau thoi gian sau khi di chuyen/sao chep
fn preserve_times(src_meta: &fs::Metadata, dest: &Path) -> std::io::Result<()> {
    let f = fs::OpenOptions::new().write(true).open(dest)?;
    let mut times = fs::FileTimes::new();
    if let Ok(m) = src_meta.modified() {
        times = times.set_modified(m);
    }
    if let Ok(a) = src_meta.accessed() {
        times = times.set_accessed(a);
    }
    f.set_times(times)
}

fn ensure_dir(dir: &Path, created: &mut HashSet<PathBuf>, jr: &mut Journal) -> std::io::Result<()> {
    if dir.as_os_str().is_empty() || dir.exists() {
        return Ok(());
    }
    // Ghi nhan tung tang moi tao de con don sach khi hoan tac
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut cur = Some(dir);
    while let Some(d) = cur {
        if d.exists() {
            break;
        }
        stack.push(d.to_path_buf());
        cur = d.parent();
    }
    fs::create_dir_all(dir)?;
    for d in stack.into_iter().rev() {
        if created.insert(d.clone()) {
            let _ = jr.record_dir(&d);
        }
    }
    Ok(())
}

/// Di chuyen an toan qua o dia khac: copy -> doi chieu hash -> xoa nguon
fn move_across_volumes(src: &Path, dest: &Path) -> std::io::Result<()> {
    let src_hash = full_hash(src);
    fs::copy(src, dest)?;

    match (src_hash, full_hash(dest)) {
        (Some(a), Some(b)) if a == b => {}
        (None, _) | (_, None) => {
            // Khong bam duoc -> doi chieu kich thuoc thay the
            let sa = fs::metadata(src)?.len();
            let sb = fs::metadata(dest)?.len();
            if sa != sb {
                let _ = fs::remove_file(dest);
                return Err(std::io::Error::other(crate::i18n::t("msg.copyMismatchSize")));
            }
        }
        _ => {
            let _ = fs::remove_file(dest);
            return Err(std::io::Error::other(crate::i18n::t("msg.copyMismatch")));
        }
    }
    fs::remove_file(src)
}

fn do_move(src: &Path, dest: &Path) -> std::io::Result<()> {
    if drive_of(src) == drive_of(dest) {
        match fs::rename(src, dest) {
            Ok(_) => Ok(()),
            // Mot so truong hop (junction, mount point) van bao khac thiet bi
            Err(e) if e.raw_os_error() == Some(17) => move_across_volumes(src, dest),
            Err(e) => Err(e),
        }
    } else {
        move_across_volumes(src, dest)
    }
}

fn move_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    if drive_of(src) == drive_of(dest) {
        fs::rename(src, dest)
    } else {
        copy_dir_recursive(src, dest)?;
        fs::remove_dir_all(src)
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let from = e.path();
        let to = dest.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ Thuc thi

pub fn execute<F: FnMut(ExecProgress)>(
    plan: &Plan,
    profile_name: &str,
    skip_ids: &HashSet<u32>,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> Result<ExecResult, String> {
    let start = std::time::Instant::now();

    let ops: Vec<&PlanOp> = plan
        .ops
        .iter()
        .filter(|o| o.selected && !matches!(o.action, OpAction::Keep | OpAction::Skip))
        .collect();

    if ops.is_empty() {
        return Err(crate::i18n::t("msg.noOps").into());
    }

    let owned: Vec<PlanOp> = ops.iter().map(|o| (*o).clone()).collect();
    let mut jr = Journal::create(
        &journal::new_session_id(),
        profile_name,
        &format!("{:?}", plan.mode),
        &plan.roots,
        owned.len(),
    )
    .map_err(|e| crate::i18n::tf("msg.journalFailed", &[&e.to_string()]))?;
    // Nhat ky co the da doi ma phien de tranh trung file — lay lai ma THUC TE dung
    let session = jr.session.clone();

    // Ghi TOAN BO ke hoach va fsync TRUOC khi dong vao bat ky file nao
    jr.write_plan(&owned)
        .map_err(|e| crate::i18n::tf("msg.journalWriteFail", &[&e.to_string()]))?;

    let mut created_dirs: HashSet<PathBuf> = HashSet::new();
    let mut res = ExecResult {
        session: session.clone(),
        ..Default::default()
    };
    let total = owned.len();

    for (seq, o) in owned.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            res.cancelled = true;
            break;
        }

        on_progress(ExecProgress {
            index: seq,
            total,
            bytes_done: res.bytes,
            current: o.src.to_string_lossy().to_string(),
        });

        if skip_ids.contains(&o.id) {
            res.skipped += 1;
            let _ = jr.skip(seq, crate::i18n::t("msg.skipByCheck"));
            continue;
        }
        if !o.src.exists() {
            res.skipped += 1;
            let _ = jr.skip(seq, crate::i18n::t("msg.srcGone"));
            continue;
        }

        let src_meta = fs::metadata(&o.src).ok();

        let outcome: std::io::Result<()> = match o.action {
            OpAction::Recycle => trash::delete(&o.src)
                .map_err(|e| std::io::Error::other(crate::i18n::tf("msg.recycleFailed", &[&e.to_string()]))),

            OpAction::Move | OpAction::Quarantine => {
                if let Some(parent) = o.dest.parent() {
                    if let Err(e) = ensure_dir(parent, &mut created_dirs, &mut jr) {
                        Err(e)
                    } else if o.is_dir {
                        move_dir(&o.src, &o.dest)
                    } else {
                        do_move(&o.src, &o.dest)
                    }
                } else {
                    Err(std::io::Error::other(crate::i18n::t("msg.badDest")))
                }
            }

            OpAction::Copy => {
                if let Some(parent) = o.dest.parent() {
                    if let Err(e) = ensure_dir(parent, &mut created_dirs, &mut jr) {
                        Err(e)
                    } else if o.is_dir {
                        copy_dir_recursive(&o.src, &o.dest)
                    } else {
                        fs::copy(&o.src, &o.dest).map(|_| ())
                    }
                } else {
                    Err(std::io::Error::other(crate::i18n::t("msg.badDest")))
                }
            }

            OpAction::Hardlink => {
                if let Some(parent) = o.dest.parent() {
                    if let Err(e) = ensure_dir(parent, &mut created_dirs, &mut jr) {
                        Err(e)
                    } else if drive_of(&o.src) != drive_of(&o.dest) {
                        Err(std::io::Error::other(crate::i18n::t("msg.hardlinkSameDrive")))
                    } else {
                        fs::hard_link(&o.src, &o.dest)
                    }
                } else {
                    Err(std::io::Error::other(crate::i18n::t("msg.badDest")))
                }
            }

            OpAction::Keep | OpAction::Skip => Ok(()),
        };

        match outcome {
            Ok(_) => {
                if !o.is_dir && o.action != OpAction::Recycle {
                    if let Some(m) = &src_meta {
                        let _ = preserve_times(m, &o.dest);
                    }
                }
                res.done += 1;
                res.bytes += o.size;
                let _ = jr.done(seq);
            }
            Err(e) => {
                res.failed += 1;
                if res.errors.len() < 200 {
                    res.errors.push(ExecError {
                        path: o.src.to_string_lossy().to_string(),
                        msg: e.to_string(),
                    });
                }
                let _ = jr.fail(seq, &e.to_string());
            }
        }
    }

    // Dọn vỏ thư mục rỗng còn lại. Chỉ làm khi thật sự có chuyển file đi
    // (chế độ sao chép hay lối tắt thì chỗ cũ vẫn còn nguyên file).
    if plan.clean_empty_dirs && plan.mode == crate::config::Mode::Move && !res.cancelled {
        on_progress(ExecProgress {
            index: total,
            total,
            bytes_done: res.bytes,
            current: crate::i18n::t("msg.sweeping").into(),
        });
        let mut removed: Vec<PathBuf> = Vec::new();
        for root in &plan.roots {
            sweep_empty_dirs(root, &mut jr, &mut removed);
        }
        res.removed_dirs = removed.len();
    }

    on_progress(ExecProgress {
        index: total,
        total,
        bytes_done: res.bytes,
        current: String::new(),
    });

    let status = if res.cancelled {
        "CANCELLED"
    } else if res.failed > 0 {
        "PARTIAL"
    } else {
        "DONE"
    };
    let _ = jr.close(status, res.done, res.failed, res.skipped);

    res.elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(res)
}

// -------------------------------------------------------------------- Hoan tac

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UndoResult {
    pub restored: usize,
    pub failed: usize,
    pub conflicts: Vec<ExecError>,
    pub removed_dirs: usize,
    /// Số thư mục đã được dựng lại đúng chỗ cũ
    pub restored_dirs: usize,
    pub elapsed_ms: u64,
}

/// Hoan tac mot phien. `only_ids` = None -> hoan tac toan bo;
/// Some(set) -> chi hoan tac cac seq duoc chon.
pub fn undo_session<F: FnMut(usize, usize)>(
    session: &str,
    only_seq: Option<HashSet<usize>>,
    mut on_progress: F,
) -> Result<UndoResult, String> {
    let start = std::time::Instant::now();
    let data = journal::read_session(session).ok_or_else(|| crate::i18n::t("msg.noSession").to_string())?;

    let mut r = UndoResult::default();

    // Gop ca thao tac chac chan da xong lan thao tac khong ro (crash giua chung).
    // Voi thao tac khong ro, ta DO tren o dia de biet no da chay hay chua.
    let mut targets: Vec<(usize, PathBuf, PathBuf, String)> = data.completed.clone();
    for u in &data.uncertain {
        let (_, src, dest, _) = u;
        if dest.exists() && !src.exists() {
            targets.push(u.clone()); // thuc te da di chuyen -> can hoan tac
        }
    }

    if let Some(sel) = &only_seq {
        targets.retain(|t| sel.contains(&t.0));
    }

    // Hoan tac theo thu tu NGUOC lai
    targets.sort_by(|a, b| b.0.cmp(&a.0));
    let total = targets.len();

    for (i, (_seq, src, dest, action)) in targets.iter().enumerate() {
        on_progress(i, total);

        // Ban da bi dua vao Thung rac -> khong the tu dong lay lai
        if action == "Recycle" {
            r.conflicts.push(ExecError {
                path: src.to_string_lossy().to_string(),
                msg: crate::i18n::t("msg.inRecycleBin").into(),
            });
            continue;
        }
        // Che do sao chep / hard link: xoa ban o dich, giu nguyen ban goc
        if action == "Copy" || action == "Hardlink" {
            if dest.exists() {
                match fs::remove_file(dest) {
                    Ok(_) => r.restored += 1,
                    Err(e) => {
                        r.failed += 1;
                        r.conflicts.push(ExecError {
                            path: dest.to_string_lossy().to_string(),
                            msg: e.to_string(),
                        });
                    }
                }
            }
            continue;
        }

        if !dest.exists() {
            r.conflicts.push(ExecError {
                path: dest.to_string_lossy().to_string(),
                msg: crate::i18n::t("msg.movedAway").into(),
            });
            continue;
        }
        if src.exists() && !same_path(src, dest) {
            r.conflicts.push(ExecError {
                path: src.to_string_lossy().to_string(),
                msg: crate::i18n::t("msg.oldSpotTaken").into(),
            });
            continue;
        }
        if let Some(parent) = src.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                r.failed += 1;
                r.conflicts.push(ExecError {
                    path: src.to_string_lossy().to_string(),
                    msg: e.to_string(),
                });
                continue;
            }
        }

        let moved = if drive_of(src) == drive_of(dest) {
            fs::rename(dest, src)
        } else {
            fs::copy(dest, src).and_then(|_| fs::remove_file(dest))
        };
        match moved {
            Ok(_) => r.restored += 1,
            Err(e) => {
                r.failed += 1;
                r.conflicts.push(ExecError {
                    path: dest.to_string_lossy().to_string(),
                    msg: e.to_string(),
                });
            }
        }
    }
    on_progress(total, total);

    // Dung lai cac vo thu muc rong ma lan don da xoa di, de tra ve dung hien trang cu
    for d in &data.removed_dirs {
        if !d.exists() && fs::create_dir_all(d).is_ok() {
            r.restored_dirs += 1;
        }
    }

    // Don thu muc rong — CHI xoa thu muc do chinh phan mem tao ra trong phien nay.
    // Ban v1 xoa moi thu muc rong, ke ca thu muc von co tu truoc.
    let mut dirs = data.created_dirs.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        if d.exists() && fs::read_dir(&d).map(|mut i| i.next().is_none()).unwrap_or(false) {
            if fs::remove_dir(&d).is_ok() {
                r.removed_dirs += 1;
            }
        }
    }

    if only_seq.is_none() {
        let _ = journal::mark_undone(session, r.restored, r.failed);
    }
    r.elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(r)
}
