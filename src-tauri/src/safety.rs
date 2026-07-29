//! Lop an toan: danh sach thu muc cam, kiem tra truoc khi chay (preflight),
//! phat hien file bi khoa, do dung luong trong.

use crate::util::{drive_of, format_bytes, is_inside, same_path};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------ Dung luong trong

#[cfg(windows)]
mod disk {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lp_directory_name: *const u16,
            lp_free_bytes_available_to_caller: *mut u64,
            lp_total_number_of_bytes: *mut u64,
            lp_total_number_of_free_bytes: *mut u64,
        ) -> i32;
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(lp_root_path_name: *const u16) -> u32;
        fn GetVolumeInformationW(
            lp_root_path_name: *const u16,
            lp_volume_name_buffer: *mut u16,
            n_volume_name_size: u32,
            lp_volume_serial_number: *mut u32,
            lp_maximum_component_length: *mut u32,
            lp_file_system_flags: *mut u32,
            lp_file_system_name_buffer: *mut u16,
            n_file_system_name_size: u32,
        ) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        let mut w: Vec<u16> = path.as_os_str().encode_wide().collect();
        w.push(0);
        w
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    /// (dung luong con trong, tong dung luong)
    pub fn space(path: &Path) -> Option<(u64, u64)> {
        let w = wide(path);
        let mut avail: u64 = 0;
        let mut total: u64 = 0;
        let ok = unsafe {
            GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, std::ptr::null_mut())
        };
        if ok != 0 {
            Some((avail, total))
        } else {
            None
        }
    }

    pub fn available(path: &Path) -> Option<u64> {
        space(path).map(|(a, _)| a)
    }

    pub fn logical_drives() -> Vec<char> {
        let mask = unsafe { GetLogicalDrives() };
        (0..26u32)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| (b'A' + i as u8) as char)
            .collect()
    }

    /// 2 = tháo rời (USB), 3 = ổ cứng trong, 4 = ổ mạng, 5 = CD/DVD, 6 = RAM disk
    pub fn drive_type(path: &Path) -> u32 {
        unsafe { GetDriveTypeW(wide(path).as_ptr()) }
    }

    /// (nhan o dia, he thong tap tin)
    pub fn volume_info(path: &Path) -> Option<(String, String)> {
        let w = wide(path);
        let mut name = [0u16; 256];
        let mut fs = [0u16; 64];
        let ok = unsafe {
            GetVolumeInformationW(
                w.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs.as_mut_ptr(),
                fs.len() as u32,
            )
        };
        if ok != 0 {
            Some((from_wide(&name), from_wide(&fs)))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
mod disk {
    use std::path::Path;
    pub fn space(_path: &Path) -> Option<(u64, u64)> {
        None
    }
    pub fn available(_path: &Path) -> Option<u64> {
        None
    }
    pub fn logical_drives() -> Vec<char> {
        vec![]
    }
    pub fn drive_type(_path: &Path) -> u32 {
        0
    }
    pub fn volume_info(_path: &Path) -> Option<(String, String)> {
        None
    }
}

pub fn free_space(path: &Path) -> Option<u64> {
    disk::available(path)
}

// ───────────────────────────────────────────────────────────────── Ổ đĩa

/// "D:\" hoac "D:" -> true. "D:\Data" -> false.
pub fn is_drive_root(p: &Path) -> bool {
    let s = p.to_string_lossy();
    let t = s.trim_end_matches(['\\', '/']);
    t.len() == 2 && t.ends_with(':') && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// Chu cai o dia chua Windows, vi du "C"
pub fn system_drive() -> String {
    std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".into())
        .trim_end_matches(['\\', ':'])
        .to_uppercase()
}

pub fn is_system_drive(p: &Path) -> bool {
    drive_of(p) == system_drive()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub letter: String,
    pub path: String,
    pub label: String,
    pub file_system: String,
    pub total: u64,
    pub free: u64,
    pub used: u64,
    /// "fixed" | "removable" | "network" | "cdrom" | "ram" | "unknown"
    pub kind: String,
    pub is_system: bool,
    /// Co the chon lam nguon quet khong
    pub selectable: bool,
    pub note: String,
}

/// Liet ke moi o dia dang gan, kem dung luong — dung cho man chon o dia.
pub fn list_drives() -> Vec<DriveInfo> {
    let sys = system_drive();
    let mut out = Vec::new();
    for letter in disk::logical_drives() {
        let root = PathBuf::from(format!("{}:\\", letter));
        let kind_code = disk::drive_type(&root);
        let kind = match kind_code {
            2 => "removable",
            3 => "fixed",
            4 => "network",
            5 => "cdrom",
            6 => "ram",
            _ => "unknown",
        };
        // O chua san sang (o quang rong, the nho chua cam) -> khong lay dung luong duoc
        let (free, total) = match disk::space(&root) {
            Some(v) => v,
            None => continue,
        };
        if total == 0 {
            continue;
        }
        let (label, fs) = disk::volume_info(&root).unwrap_or_default();
        let is_system = letter.to_string().to_uppercase() == sys;

        let (selectable, note) = if is_system {
            (
                false,
                crate::i18n::t("msg.sysDriveShort").to_string(),
            )
        } else if kind == "cdrom" {
            (false, crate::i18n::t("msg.cdromShort").to_string())
        } else if kind == "network" {
            (
                true,
                crate::i18n::t("msg.netDrive").to_string(),
            )
        } else {
            (true, String::new())
        };

        out.push(DriveInfo {
            letter: letter.to_string(),
            path: root.to_string_lossy().to_string(),
            label: if label.is_empty() {
                match kind {
                    "removable" => crate::i18n::t("ui.driveRemovable").into(),
                    "network" => crate::i18n::t("ui.driveNetwork").into(),
                    _ => crate::i18n::t("ui.driveLocal").into(),
                }
            } else {
                label
            },
            file_system: fs,
            total,
            free,
            used: total.saturating_sub(free),
            kind: kind.to_string(),
            is_system,
            selectable,
            note,
        });
    }
    out
}

// ------------------------------------------------------------ Thu muc duoc bao ve

pub fn hard_block_list() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for key in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Ok(p) = std::env::var(key) {
            v.push(PathBuf::from(p));
        }
    }
    if let Ok(p) = std::env::var("USERPROFILE") {
        v.push(PathBuf::from(p).join("AppData"));
    }
    v
}

fn warn_list() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_default();
    vec![
        home.join("Desktop"),
        home.join("Documents"),
        home.clone(),
        home,
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathCheck {
    pub ok: bool,
    pub level: String, // "ok" | "warn" | "block"
    pub reason: String,
}

/// Kiem tra thu muc nguon co duoc phep thao tac khong
pub fn check_source(p: &Path) -> PathCheck {
    let block = |r: &str| PathCheck {
        ok: false,
        level: "block".into(),
        reason: r.into(),
    };

    let md = match fs::metadata(p) {
        Ok(m) => m,
        Err(e) => return block(&crate::i18n::tf("msg.cantRead", &[&format!("{}", e.kind())])),
    };
    if !md.is_dir() {
        return block(crate::i18n::t("msg.notFolder"));
    }

    // Goc o dia. O chua Windows thi cam tuyet doi; phan vung du lieu thi cho,
    // nhung kem canh bao va cac lop bao ve rieng o `scanner` (xem DRIVE_ROOT_PROTECTED).
    if is_drive_root(p) {
        if is_system_drive(p) {
            return block(crate::i18n::t("msg.sysDrive"));
        }
        let kind = disk::drive_type(p);
        if kind == 5 {
            return block(crate::i18n::t("msg.cdrom"));
        }
        return PathCheck {
            ok: true,
            level: "warn".into(),
            reason: crate::i18n::tf("msg.wholeDrive", &[&p.to_string_lossy()]),
        };
    }

    for b in hard_block_list() {
        if same_path(p, &b) || is_inside(p, &b) {
            return block(&crate::i18n::tf("msg.protectedDir", &[&b.to_string_lossy()]));
        }
    }
    for w in warn_list() {
        if same_path(p, &w) {
            return PathCheck {
                ok: true,
                level: "warn".into(),
                reason: crate::i18n::tf("msg.importantDir", &[&w
                    .file_name()
                    .map(|x| x.to_string_lossy().to_string())
                    .unwrap_or_else(|| w.to_string_lossy().to_string())]),
            };
        }
    }
    PathCheck {
        ok: true,
        level: "ok".into(),
        reason: String::new(),
    }
}

/// Thu muc dich khong duoc chua thu muc nguon (se gay de quy vo han)
pub fn check_destination(source: &Path, dest: &Path) -> PathCheck {
    if same_path(source, dest) {
        return PathCheck {
            ok: true,
            level: "ok".into(),
            reason: String::new(),
        };
    }
    if is_inside(source, dest) {
        return PathCheck {
            ok: false,
            level: "block".into(),
            reason: crate::i18n::t("msg.recursion").into(),
        };
    }
    if is_inside(dest, source) {
        return PathCheck {
            ok: true,
            level: "warn".into(),
            reason: crate::i18n::t("msg.destInside").into(),
        };
    }
    PathCheck {
        ok: true,
        level: "ok".into(),
        reason: String::new(),
    }
}

// ----------------------------------------------------------------- File bi khoa

/// Thu mo file voi quyen ghi de biet no co dang bi ung dung khac giu khong.
/// `None` = on. `Some(mo_ta)` = dang bi khoa.
pub fn probe_lock(path: &Path) -> Option<String> {
    match fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => None,
        Err(e) => match e.raw_os_error() {
            // ERROR_SHARING_VIOLATION (32) / ERROR_LOCK_VIOLATION (33)
            Some(32) | Some(33) => Some("đang được ứng dụng khác mở".into()),
            Some(5) => Some("không có quyền truy cập".into()),
            _ => match e.kind() {
                std::io::ErrorKind::PermissionDenied => Some("không có quyền truy cập".into()),
                _ => None,
            },
        },
    }
}

/// Thu tao file tam de xac nhan co quyen ghi
pub fn can_write(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".foldu-write-test-{}", std::process::id()));
    match fs::write(&probe, b"x") {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

// ------------------------------------------------------------------- Preflight

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub level: String, // "error" | "warn" | "info"
    pub code: String,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResult {
    pub ok: bool,
    pub issues: Vec<Issue>,
    /// Id cac thao tac phai bo qua
    pub skip_ids: Vec<u32>,
    pub est_bytes: u64,
}

pub struct PreflightOp {
    pub id: u32,
    pub src: PathBuf,
    pub dest: PathBuf,
    pub size: u64,
}

pub fn preflight(ops: &[PreflightOp], mode_copies: bool, roots: &[PathBuf]) -> PreflightResult {
    let mut issues = Vec::new();
    let mut skip_ids: Vec<u32> = Vec::new();

    if ops.is_empty() {
        return PreflightResult {
            ok: false,
            issues: vec![Issue {
                level: "error".into(),
                code: "EMPTY".into(),
                msg: crate::i18n::t("msg.nothingPicked").into(),
            }],
            skip_ids,
            est_bytes: 0,
        };
    }

    // 1. Quyen ghi tai thu muc goc
    for r in roots {
        if !can_write(r) {
            issues.push(Issue {
                level: "error".into(),
                code: "NOWRITE".into(),
                msg: crate::i18n::tf("msg.noWrite", &[&r.to_string_lossy()]),
            });
        }
    }

    // 2. Duong dan dich qua dai
    let too_long: Vec<u32> = ops
        .iter()
        .filter(|o| crate::util::is_path_too_long(&o.dest))
        .map(|o| o.id)
        .collect();
    if !too_long.is_empty() {
        issues.push(Issue {
            level: "warn".into(),
            code: "LONGPATH".into(),
            msg: crate::i18n::tf("msg.longPath", &[&too_long.len().to_string()]),
        });
        skip_ids.extend(too_long);
    }

    // 3. File dang bi khoa (lay mau neu qua nhieu)
    let sample: Vec<&PreflightOp> = if ops.len() > 3000 {
        ops.iter().take(3000).collect()
    } else {
        ops.iter().collect()
    };
    let mut locked = 0usize;
    for o in sample {
        if probe_lock(&o.src).is_some() {
            locked += 1;
            skip_ids.push(o.id);
        }
    }
    if locked > 0 {
        issues.push(Issue {
            level: "warn".into(),
            code: "LOCKED".into(),
            msg: crate::i18n::tf("msg.locked", &[&locked.to_string()]),
        });
    }

    // 4. Dung luong trong (chi can khi COPY hoac di chuyen khac o dia)
    let mut need_by_drive: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut est_bytes = 0u64;
    for o in ops {
        if skip_ids.contains(&o.id) {
            continue;
        }
        est_bytes += o.size;
        let cross = drive_of(&o.src) != drive_of(&o.dest);
        if mode_copies || cross {
            *need_by_drive.entry(drive_of(&o.dest)).or_insert(0) += o.size;
        }
    }
    for (drive, need) in &need_by_drive {
        let root = PathBuf::from(format!("{}:\\", drive));
        if let Some(free) = free_space(&root) {
            let want = (*need as f64 * 1.1) as u64;
            if free < want {
                issues.push(Issue {
                    level: "error".into(),
                    code: "NOSPACE".into(),
                    msg: crate::i18n::tf("msg.noSpace", &[drive, &format_bytes(want), &format_bytes(free)]),
                });
            }
        }
    }

    // 5. Trung duong dan dich (loi logic nghiem trong — khong bao gio duoc xay ra)
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut clash = 0usize;
    for o in ops {
        let k = o.dest.to_string_lossy().to_lowercase();
        match seen.get(&k) {
            Some(&other) if other != o.id => clash += 1,
            _ => {
                seen.insert(k, o.id);
            }
        }
    }
    if clash > 0 {
        issues.push(Issue {
            level: "error".into(),
            code: "DESTCLASH".into(),
            msg: crate::i18n::tf("msg.destClash", &[&clash.to_string()]),
        });
    }

    skip_ids.sort_unstable();
    skip_ids.dedup();
    let ok = !issues.iter().any(|i| i.level == "error");
    PreflightResult {
        ok,
        issues,
        skip_ids,
        est_bytes,
    }
}
