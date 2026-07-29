//! Quet cay thu muc: doc metadata co ban, ap bo loc, phat hien thu muc du an.
//! Metadata nang (EXIF, kich thuoc anh) duoc doc lazy o buoc lap ke hoach.

use crate::config::{Filters, Profile, PROJECT_MARKERS, PROJECT_MARKER_EXTS};
use crate::util::{ext_of, is_inside};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ------------------------------------------------------- Thuoc tinh Windows

#[cfg(windows)]
mod attrs {
    pub const HIDDEN: u32 = 0x0000_0002;
    pub const SYSTEM: u32 = 0x0000_0004;
    pub const REPARSE_POINT: u32 = 0x0000_0400;
    pub const OFFLINE: u32 = 0x0000_1000;
    pub const RECALL_ON_OPEN: u32 = 0x0004_0000;
    pub const RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
}

#[cfg(windows)]
fn file_attributes(md: &fs::Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    md.file_attributes()
}
#[cfg(not(windows))]
fn file_attributes(_md: &fs::Metadata) -> u32 {
    0
}

#[cfg(windows)]
pub fn is_hidden(md: &fs::Metadata) -> bool {
    file_attributes(md) & attrs::HIDDEN != 0
}
#[cfg(not(windows))]
pub fn is_hidden(_md: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
pub fn is_system(md: &fs::Metadata) -> bool {
    file_attributes(md) & attrs::SYSTEM != 0
}
#[cfg(not(windows))]
pub fn is_system(_md: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
pub fn is_reparse_point(md: &fs::Metadata) -> bool {
    file_attributes(md) & attrs::REPARSE_POINT != 0
}
#[cfg(not(windows))]
pub fn is_reparse_point(md: &fs::Metadata) -> bool {
    md.file_type().is_symlink()
}

/// File dam may (OneDrive / Google Drive / Dropbox) chua tai noi dung ve may.
/// Doc no se kich hoat tai xuong - mac dinh phai BO QUA.
#[cfg(windows)]
pub fn is_cloud_placeholder(md: &fs::Metadata) -> bool {
    let a = file_attributes(md);
    a & (attrs::OFFLINE | attrs::RECALL_ON_OPEN | attrs::RECALL_ON_DATA_ACCESS) != 0
}
#[cfg(not(windows))]
pub fn is_cloud_placeholder(_md: &fs::Metadata) -> bool {
    false
}

// -------------------------------------------------------------------- Glob

/// Chuyen glob don gian (* va **) thanh mot bo so khop tren chuoi thuong.
/// Mau khong chua dau "/" duoc so khop voi TEN FILE; mau co "/" so khop voi CA DUONG DAN.
#[derive(Debug, Clone)]
pub struct Ignore {
    exact: Vec<String>,
    name_globs: Vec<Vec<GlobPart>>,
    path_globs: Vec<Vec<GlobPart>>,
}

#[derive(Debug, Clone, PartialEq)]
enum GlobPart {
    Literal(String),
    Any,      // *  (khong vuot dau /)
    AnyDeep,  // ** (vuot moi thu)
}

fn parse_glob(pat: &str) -> Vec<GlobPart> {
    let p = pat.to_lowercase().replace('\\', "/");
    let mut parts = Vec::new();
    let mut lit = String::new();
    let bytes: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '*' {
            if !lit.is_empty() {
                parts.push(GlobPart::Literal(std::mem::take(&mut lit)));
            }
            if i + 1 < bytes.len() && bytes[i + 1] == '*' {
                parts.push(GlobPart::AnyDeep);
                i += 2;
                if i < bytes.len() && bytes[i] == '/' {
                    i += 1;
                }
            } else {
                parts.push(GlobPart::Any);
                i += 1;
            }
        } else {
            lit.push(bytes[i]);
            i += 1;
        }
    }
    if !lit.is_empty() {
        parts.push(GlobPart::Literal(lit));
    }
    parts
}

fn glob_match(parts: &[GlobPart], text: &str) -> bool {
    match parts.first() {
        None => text.is_empty(),
        Some(GlobPart::Literal(l)) => text
            .strip_prefix(l.as_str())
            .map_or(false, |rest| glob_match(&parts[1..], rest)),
        Some(GlobPart::Any) => {
            // Thu moi diem cat, nhung khong duoc vuot qua dau '/'
            let mut idx = 0usize;
            loop {
                if glob_match(&parts[1..], &text[idx..]) {
                    return true;
                }
                match text[idx..].chars().next() {
                    None | Some('/') => return false,
                    Some(c) => idx += c.len_utf8(),
                }
            }
        }
        Some(GlobPart::AnyDeep) => {
            let mut idx = 0usize;
            loop {
                if glob_match(&parts[1..], &text[idx..]) {
                    return true;
                }
                match text[idx..].chars().next() {
                    None => return false,
                    Some(c) => idx += c.len_utf8(),
                }
            }
        }
    }
}

impl Ignore {
    pub fn new(patterns: &[String]) -> Self {
        let mut exact = Vec::new();
        let mut name_globs = Vec::new();
        let mut path_globs = Vec::new();
        for p in patterns {
            let s = p.to_lowercase().replace('\\', "/");
            let has_sep = s.contains('/');
            if s.contains('*') {
                if has_sep {
                    path_globs.push(parse_glob(&s));
                } else {
                    name_globs.push(parse_glob(&s));
                }
            } else if has_sep {
                path_globs.push(parse_glob(&s));
            } else {
                exact.push(s);
            }
        }
        Ignore {
            exact,
            name_globs,
            path_globs,
        }
    }

    pub fn matches(&self, name: &str, full: &Path) -> bool {
        let n = name.to_lowercase();
        if self.exact.iter().any(|e| *e == n) {
            return true;
        }
        if self.name_globs.iter().any(|g| glob_match(g, &n)) {
            return true;
        }
        if self.path_globs.is_empty() {
            return false;
        }
        let f = full.to_string_lossy().to_lowercase().replace('\\', "/");
        self.path_globs.iter().any(|g| glob_match(g, &f))
    }
}

// ------------------------------------------------------------------ Ket qua

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: u32,
    pub path: PathBuf,
    pub name: String,
    pub root: PathBuf,
    /// true khi day la mot thu muc du an duoc coi nhu MOT don vi
    pub is_dir: bool,
    pub project_marker: Option<String>,
    pub ext: String,
    pub size: u64,
    pub mtime: i64,
    pub ctime: i64,
    pub atime: i64,
    pub parent: PathBuf,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedItem {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub total_files: usize,
    pub total_bytes: u64,
    pub scanned_dirs: usize,
    pub project_folders: usize,
    pub cloud_skipped: usize,
    /// Thu muc ung dung da duoc de nguyen tai cho
    pub app_folders_protected: usize,
    /// Muc he thong o goc o dia da duoc bo qua vo dieu kien
    pub system_protected: usize,
    /// Dang quet nguyen mot phan vung
    pub whole_drive: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub files: Vec<FileEntry>,
    pub skipped: Vec<SkippedItem>,
    pub stats: ScanStats,
}

fn to_ms(t: std::io::Result<SystemTime>) -> i64 {
    t.ok()
        .and_then(|s| s.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ------------------------------------------------------- Nhan dien thu muc du an

/// Nhan dien thu muc chua UNG DUNG da cai hoac ban portable.
/// Di chuyen mot thu muc nhu vay se lam hong duong dan trong registry, shortcut
/// va cac tep cau hinh tro tuyet doi — nen mac dinh phai de nguyen tai cho.
/// Tra ve ly do de con giai thich cho nguoi dung.
fn app_folder_reason(entries: &[fs::DirEntry]) -> Option<String> {
    let mut has_uninstaller = false;
    let mut special: Option<&str> = None;
    let mut exe = 0usize;
    let mut dll = 0usize;

    for e in entries {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if n.starts_with("unins") && n.ends_with(".exe") {
            has_uninstaller = true;
        }
        match n.as_str() {
            "steamapps" | "steam.exe" => special = Some("thư viện Steam"),
            "app.asar" | "resources.pak" | "icudtl.dat" | "chrome_100_percent.pak" => {
                special = Some("ứng dụng Electron/Chromium")
            }
            "manifest.json" if exe > 0 => special = Some("ứng dụng đóng gói"),
            _ => {}
        }
        if n.ends_with(".exe") {
            exe += 1;
        } else if n.ends_with(".dll") || n.ends_with(".pyd") || n.ends_with(".node") {
            dll += 1;
        }
    }

    if has_uninstaller {
        return Some("có trình gỡ cài đặt".into());
    }
    if let Some(s) = special {
        return Some(s.into());
    }
    // Mot chuong trinh thuc thu luon di kem thu vien lien ket dong
    if exe >= 1 && dll >= 3 {
        return Some(format!("{} tệp .exe kèm {} thư viện .dll", exe, dll));
    }
    None
}

fn project_marker_of(entries: &[fs::DirEntry]) -> Option<String> {
    let names: Vec<String> = entries
        .iter()
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect();
    for m in PROJECT_MARKERS {
        if names.iter().any(|n| n == m) {
            return Some((*m).to_string());
        }
    }
    for e in PROJECT_MARKER_EXTS {
        let suffix = format!(".{}", e);
        if names.iter().any(|n| n.ends_with(&suffix)) {
            return Some(format!("*.{}", e));
        }
    }
    None
}

// ------------------------------------------------------------------ Quet chinh

pub struct Scanner<'a> {
    filters: &'a Filters,
    ignore: Ignore,
    whitelist: Vec<String>,
    blacklist: Vec<String>,
    recursive: bool,
    treat_projects: bool,
    protect_apps: bool,
    exclude: Vec<PathBuf>,
    /// Goc dang duyet co phai nguyen mot phan vung khong
    root_is_drive: bool,

    files: Vec<FileEntry>,
    skipped: Vec<SkippedItem>,
    stats: ScanStats,
    next_id: u32,
}

impl<'a> Scanner<'a> {
    pub fn new(profile: &'a Profile, exclude: Vec<PathBuf>) -> Self {
        let f = &profile.filters;
        Scanner {
            filters: f,
            ignore: Ignore::new(&f.ignore_patterns),
            whitelist: f
                .ext_whitelist
                .iter()
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .collect(),
            blacklist: f
                .ext_blacklist
                .iter()
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .collect(),
            recursive: profile.recursive,
            treat_projects: profile.safety.treat_project_folders_as_unit,
            protect_apps: profile.safety.protect_installed_apps,
            exclude,
            root_is_drive: false,
            files: Vec::new(),
            skipped: Vec::new(),
            stats: ScanStats::default(),
            next_id: 0,
        }
    }

    fn skip(&mut self, path: &Path, reason: &str) {
        // Gioi han so muc ghi nhan de khong phinh bo nho
        if self.skipped.len() < 5000 {
            self.skipped.push(SkippedItem {
                path: path.to_path_buf(),
                reason: reason.to_string(),
            });
        }
    }

    pub fn run<F: FnMut(usize, &Path)>(
        mut self,
        roots: &[PathBuf],
        mut on_progress: F,
    ) -> ScanResult {
        let start = std::time::Instant::now();
        for r in roots {
            let root = r.clone();
            self.root_is_drive = crate::safety::is_drive_root(&root);
            if self.root_is_drive {
                self.stats.whole_drive = true;
            }
            self.walk(&root, &root, 0, &mut on_progress);
        }
        self.stats.total_files = self.files.len();
        self.stats.total_bytes = self.files.iter().map(|f| f.size).sum();
        self.stats.project_folders = self.files.iter().filter(|f| f.is_dir).count();
        self.stats.elapsed_ms = start.elapsed().as_millis() as u64;
        ScanResult {
            files: self.files,
            skipped: self.skipped,
            stats: self.stats,
        }
    }

    fn walk<F: FnMut(usize, &Path)>(
        &mut self,
        dir: &Path,
        root: &Path,
        depth: usize,
        on_progress: &mut F,
    ) {
        if depth > 64 {
            self.skip(dir, crate::i18n::t("msg.skipDeep"));
            return;
        }

        let entries: Vec<fs::DirEntry> = match fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(e) => {
                self.skip(dir, &format!("Khong doc duoc thu muc ({})", e.kind()));
                return;
            }
        };
        self.stats.scanned_dirs += 1;

        // Thu muc UNG DUNG -> de nguyen tai cho, khong duyet vao trong, khong di chuyen.
        // Kiem tra truoc thu muc du an vi mot app dong goi cung co the chua package.json.
        if self.protect_apps && depth > 0 {
            if let Some(reason) = app_folder_reason(&entries) {
                self.stats.app_folders_protected += 1;
                self.skip(
                    dir,
                    &crate::i18n::tf("msg.skipApp", &[&reason]),
                );
                return;
            }
        }

        // Thu muc du an -> coi ca thu muc la MOT don vi, khong duyet vao trong
        if self.treat_projects && depth > 0 {
            if let Some(marker) = project_marker_of(&entries) {
                let md = fs::metadata(dir).ok();
                let (size, count) = dir_rough_size(&entries);
                let _ = count;
                let id = self.next_id;
                self.next_id += 1;
                self.files.push(FileEntry {
                    id,
                    path: dir.to_path_buf(),
                    name: dir
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    root: root.to_path_buf(),
                    is_dir: true,
                    project_marker: Some(marker),
                    ext: String::new(),
                    size,
                    mtime: md.as_ref().map(|m| to_ms(m.modified())).unwrap_or(0),
                    ctime: md.as_ref().map(|m| to_ms(m.created())).unwrap_or(0),
                    atime: md.as_ref().map(|m| to_ms(m.accessed())).unwrap_or(0),
                    parent: dir.parent().unwrap_or(dir).to_path_buf(),
                    depth,
                });
                return;
            }
        }

        for e in &entries {
            let full = e.path();
            let name = e.file_name().to_string_lossy().to_string();

            // Bao ve VO DIEU KIEN cac muc he thong nam ngay goc phan vung.
            // Khong phu thuoc vao bo loc "hiện file ẩn / hệ thống" cua nguoi dung —
            // dong vao day co the lam hong o dia hoac he dieu hanh.
            if self.root_is_drive && depth == 0 {
                let lower = name.to_lowercase();
                if crate::config::DRIVE_ROOT_PROTECTED.contains(&lower.as_str()) {
                    self.stats.system_protected += 1;
                        self.skip(&full, crate::i18n::t("msg.skipSystem"));
                    continue;
                }
            }

            if self.ignore.matches(&name, &full) {
                continue;
            }
            if self
                .exclude
                .iter()
                .any(|x| full == *x || is_inside(&full, x))
            {
                continue;
            }

            let md = match e.metadata() {
                Ok(m) => m,
                Err(err) => {
                    self.skip(&full, &format!("Khong doc duoc thong tin ({})", err.kind()));
                    continue;
                }
            };

            // Lien ket tuong trung / junction -> khong bao gio di theo (chong vong lap)
            if is_reparse_point(&md) {
                self.skip(&full, crate::i18n::t("msg.skipLink"));
                continue;
            }

            if md.is_dir() {
                if self.recursive {
                    self.walk(&full, root, depth + 1, on_progress);
                }
                continue;
            }
            if !md.is_file() {
                continue;
            }

            if !self.filters.include_hidden && is_hidden(&md) {
                continue;
            }
            if !self.filters.include_system && is_system(&md) {
                continue;
            }
            if self.filters.skip_cloud_placeholder && is_cloud_placeholder(&md) {
                self.stats.cloud_skipped += 1;
                self.skip(&full, crate::i18n::t("msg.skipCloud"));
                continue;
            }

            let ext = ext_of(&name);
            if !self.whitelist.is_empty() && !self.whitelist.contains(&ext) {
                continue;
            }
            if self.blacklist.contains(&ext) {
                continue;
            }
            let size = md.len();
            if self.filters.min_size_bytes > 0 && size < self.filters.min_size_bytes {
                continue;
            }
            if self.filters.max_size_bytes > 0 && size > self.filters.max_size_bytes {
                continue;
            }

            let id = self.next_id;
            self.next_id += 1;
            self.files.push(FileEntry {
                id,
                path: full.clone(),
                name,
                root: root.to_path_buf(),
                is_dir: false,
                project_marker: None,
                ext,
                size,
                mtime: to_ms(md.modified()),
                ctime: to_ms(md.created()),
                atime: to_ms(md.accessed()),
                parent: dir.to_path_buf(),
                depth,
            });

            if self.files.len() % 512 == 0 {
                on_progress(self.files.len(), &full);
            }
        }
    }
}

fn dir_rough_size(entries: &[fs::DirEntry]) -> (u64, usize) {
    let mut size = 0u64;
    let mut count = 0usize;
    for e in entries {
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                size += m.len();
                count += 1;
            }
        }
    }
    (size, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_basic() {
        let ig = Ignore::new(&[
            "thumbs.db".to_string(),
            "**/node_modules/**".to_string(),
            "*.tmp".to_string(),
        ]);
        assert!(ig.matches("Thumbs.db", Path::new("C:\\a\\Thumbs.db")));
        assert!(ig.matches("x.js", Path::new("C:\\a\\node_modules\\x.js")));
        assert!(ig.matches("a.tmp", Path::new("C:\\a\\a.tmp")));
        assert!(!ig.matches("a.txt", Path::new("C:\\a\\a.txt")));
    }
}
