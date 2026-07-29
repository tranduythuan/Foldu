//! Write-Ahead Journal — nhat ky ghi TRUOC khi dong vao file.
//!
//! Day la khac biet quan trong nhat so voi ban v1 (ghi history.json SAU khi
//! don xong): neu mat dien hoac crash giua chung, ban v1 mat sach kha nang
//! hoan tac. O day toan bo ke hoach duoc ghi va fsync TRUOC khi thao tac dau
//! tien chay, nen luon khoi phuc duoc.
//!
//! Dinh dang JSONL (moi dong mot ban ghi) — ghi noi tiep, khong bao gio sua
//! dong cu, nen mot dong hong khong lam hong ca file.

use crate::config::{ensure_dirs, journal_dir};
use crate::planner::{OpAction, PlanOp};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Record {
    #[serde(rename = "START")]
    Start {
        session: String,
        started_at: i64,
        profile: String,
        mode: String,
        roots: Vec<String>,
        total: usize,
    },
    /// Ghi TRUOC khi thao tac — day la ban ghi cuu ho
    #[serde(rename = "PLANNED")]
    Planned {
        seq: usize,
        src: String,
        dest: String,
        size: u64,
        action: String,
    },
    /// Thu muc do phan mem tao ra (de don sach khi hoan tac)
    #[serde(rename = "DIR")]
    Dir { path: String },
    /// Vo thu muc rong da bi don di sau khi chuyen het file ra (de dung lai khi hoan tac)
    #[serde(rename = "RMDIR")]
    RmDir { path: String },
    #[serde(rename = "DONE")]
    Done { seq: usize },
    #[serde(rename = "FAIL")]
    Fail { seq: usize, err: String },
    #[serde(rename = "SKIP")]
    Skip { seq: usize, reason: String },
    #[serde(rename = "END")]
    End {
        status: String,
        finished_at: i64,
        done: usize,
        failed: usize,
        skipped: usize,
    },
    /// Danh dau phien da duoc hoan tac
    #[serde(rename = "UNDONE")]
    Undone { at: i64, restored: usize, failed: usize },
}

pub struct Journal {
    pub session: String,
    path: PathBuf,
    writer: BufWriter<File>,
    since_flush: usize,
}

static SESSION_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Ma phien phai DUY NHAT tuyet doi: hai phien ghi chung mot file nhat ky se tron
/// thao tac cua nhau va lam hong hoan toan kha nang hoan tac.
/// Do phan giai toi mili giay + PID + bo dem tang dan trong tien trinh.
pub fn new_session_id() -> String {
    let now = chrono::Local::now();
    let n = SESSION_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(
        "{}{:03}-{}-{}",
        now.format("%Y%m%d-%H%M%S"),
        now.timestamp_subsec_millis(),
        std::process::id(),
        n
    )
}

impl Journal {
    pub fn create(
        session: &str,
        profile: &str,
        mode: &str,
        roots: &[PathBuf],
        total: usize,
    ) -> std::io::Result<Self> {
        ensure_dirs();
        // `create_new` la bat buoc: neu file da ton tai nghia la ma phien bi trung,
        // va ghi de len nhat ky cu se pha huy kha nang hoan tac cua phien do.
        let base = session.to_string();
        let (file, session, path) = {
            let mut found = None;
            for attempt in 0..64u32 {
                let id = if attempt == 0 {
                    base.clone()
                } else {
                    format!("{}-r{}", base, attempt)
                };
                let p = journal_dir().join(format!("{}.jsonl", id));
                match OpenOptions::new().create_new(true).write(true).open(&p) {
                    Ok(f) => {
                        found = Some((f, id, p));
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e),
                }
            }
            found.ok_or_else(|| {
                std::io::Error::other(crate::i18n::t("msg.journalRetry"))
            })?
        };

        let mut j = Journal {
            session: session.clone(),
            path,
            writer: BufWriter::new(file),
            since_flush: 0,
        };
        j.write(&Record::Start {
            session,
            started_at: crate::util::now_ms(),
            profile: profile.to_string(),
            mode: mode.to_string(),
            roots: roots.iter().map(|r| r.to_string_lossy().to_string()).collect(),
            total,
        })?;
        Ok(j)
    }

    fn write(&mut self, r: &Record) -> std::io::Result<()> {
        let line = serde_json::to_string(r).unwrap_or_default();
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.since_flush += 1;
        Ok(())
    }

    /// Ghi TOAN BO ke hoach roi fsync — sau lenh nay, du mat dien cung khoi phuc duoc
    pub fn write_plan(&mut self, ops: &[PlanOp]) -> std::io::Result<()> {
        for (seq, o) in ops.iter().enumerate() {
            self.write(&Record::Planned {
                seq,
                src: o.src.to_string_lossy().to_string(),
                dest: o.dest.to_string_lossy().to_string(),
                size: o.size,
                action: format!("{:?}", o.action),
            })?;
        }
        self.sync()
    }

    pub fn record_dir(&mut self, p: &Path) -> std::io::Result<()> {
        self.write(&Record::Dir {
            path: p.to_string_lossy().to_string(),
        })
    }

    pub fn record_removed_dir(&mut self, p: &Path) -> std::io::Result<()> {
        self.write(&Record::RmDir {
            path: p.to_string_lossy().to_string(),
        })
    }

    pub fn done(&mut self, seq: usize) -> std::io::Result<()> {
        self.write(&Record::Done { seq })?;
        self.maybe_flush()
    }
    pub fn fail(&mut self, seq: usize, err: &str) -> std::io::Result<()> {
        self.write(&Record::Fail {
            seq,
            err: err.to_string(),
        })?;
        self.maybe_flush()
    }
    pub fn skip(&mut self, seq: usize, reason: &str) -> std::io::Result<()> {
        self.write(&Record::Skip {
            seq,
            reason: reason.to_string(),
        })?;
        self.maybe_flush()
    }

    fn maybe_flush(&mut self) -> std::io::Result<()> {
        if self.since_flush >= 200 {
            self.sync()?;
        }
        Ok(())
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        self.since_flush = 0;
        Ok(())
    }

    pub fn close(
        mut self,
        status: &str,
        done: usize,
        failed: usize,
        skipped: usize,
    ) -> std::io::Result<PathBuf> {
        self.write(&Record::End {
            status: status.to_string(),
            finished_at: crate::util::now_ms(),
            done,
            failed,
            skipped,
        })?;
        self.sync()?;
        Ok(self.path.clone())
    }
}

// ------------------------------------------------------------------- Doc lai

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub profile: String,
    pub mode: String,
    pub roots: Vec<String>,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    /// RUNNING (phien do dang do crash) | DONE | UNDONE | FAILED
    pub status: String,
    pub can_undo: bool,
}

#[derive(Debug, Clone)]
pub struct SessionData {
    pub info: SessionInfo,
    /// (seq, src, dest, action) cua cac thao tac DA THUC HIEN
    pub completed: Vec<(usize, PathBuf, PathBuf, String)>,
    /// Cac thao tac da ghi ke hoach nhung chua ro ket qua (crash giua chung)
    pub uncertain: Vec<(usize, PathBuf, PathBuf, String)>,
    pub created_dirs: Vec<PathBuf>,
    /// Vo thu muc rong da don di, can dung lai khi hoan tac
    pub removed_dirs: Vec<PathBuf>,
}

pub fn read_session(session: &str) -> Option<SessionData> {
    let path = journal_dir().join(format!("{}.jsonl", session));
    let file = File::open(&path).ok()?;
    let reader = BufReader::new(file);

    let mut info = SessionInfo {
        session: session.to_string(),
        status: "RUNNING".into(),
        ..Default::default()
    };
    let mut planned: Vec<(usize, PathBuf, PathBuf, String)> = Vec::new();
    let mut done_set: std::collections::HashSet<usize> = Default::default();
    let mut resolved: std::collections::HashSet<usize> = Default::default();
    let mut created_dirs: Vec<PathBuf> = Vec::new();
    let mut removed_dirs: Vec<PathBuf> = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let rec: Record = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue, // dong hong -> bo qua, khong lam hong ca file
        };
        match rec {
            Record::Start {
                started_at,
                profile,
                mode,
                roots,
                total,
                ..
            } => {
                info.started_at = started_at;
                info.profile = profile;
                info.mode = mode;
                info.roots = roots;
                info.total = total;
            }
            Record::Planned {
                seq,
                src,
                dest,
                action,
                ..
            } => planned.push((seq, PathBuf::from(src), PathBuf::from(dest), action)),
            Record::Dir { path } => created_dirs.push(PathBuf::from(path)),
            Record::RmDir { path } => removed_dirs.push(PathBuf::from(path)),
            Record::Done { seq } => {
                done_set.insert(seq);
                resolved.insert(seq);
            }
            Record::Fail { seq, .. } => {
                info.failed += 1;
                resolved.insert(seq);
            }
            Record::Skip { seq, .. } => {
                info.skipped += 1;
                resolved.insert(seq);
            }
            Record::End {
                status,
                finished_at,
                done,
                failed,
                skipped,
            } => {
                info.status = status;
                info.finished_at = finished_at;
                info.done = done;
                info.failed = failed;
                info.skipped = skipped;
            }
            Record::Undone { at, .. } => {
                info.status = "UNDONE".into();
                info.finished_at = at;
            }
        }
    }

    let mut completed = Vec::new();
    let mut uncertain = Vec::new();
    for p in planned {
        if done_set.contains(&p.0) {
            completed.push(p);
        } else if !resolved.contains(&p.0) {
            uncertain.push(p);
        }
    }
    if info.done == 0 {
        info.done = completed.len();
    }
    info.can_undo = info.status != "UNDONE" && (!completed.is_empty() || !uncertain.is_empty());

    Some(SessionData {
        info,
        completed,
        uncertain,
        created_dirs,
        removed_dirs,
    })
}

pub fn list_sessions() -> Vec<SessionInfo> {
    ensure_dirs();
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(journal_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_suffix(".jsonl") {
                if let Some(d) = read_session(id) {
                    out.push(d.info);
                }
            }
        }
    }
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    out
}

/// Cac phien bi ngat giua chung (khong co ban ghi END) — hien banner khi mo app
pub fn interrupted_sessions() -> Vec<SessionInfo> {
    list_sessions()
        .into_iter()
        .filter(|s| s.status == "RUNNING")
        .collect()
}

pub fn mark_undone(session: &str, restored: usize, failed: usize) -> std::io::Result<()> {
    let path = journal_dir().join(format!("{}.jsonl", session));
    let mut f = OpenOptions::new().append(true).open(path)?;
    let rec = Record::Undone {
        at: crate::util::now_ms(),
        restored,
        failed,
    };
    writeln!(f, "{}", serde_json::to_string(&rec).unwrap_or_default())?;
    f.sync_data()
}

/// Chuoi mo ta hanh dong dung trong journal
pub fn action_str(a: OpAction) -> String {
    format!("{:?}", a)
}
