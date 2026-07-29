//! Tien ich chung: chuan hoa tieng Viet, lam sach ten thu muc, dinh dang so lieu.

use chrono::{DateTime, Datelike, Local, TimeZone};
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

// ------------------------------------------------------------------ Tieng Viet

/// Bo dau tieng Viet: "Báo cáo Đông" -> "Bao cao Dong"
pub fn strip_diacritics(s: &str) -> String {
    s.nfd()
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F))
        .map(|c| match c {
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        })
        .collect()
}

/// Khoa so sanh: bo dau + chu thuong + gom khoang trang
pub fn norm_key(s: &str) -> String {
    let stripped = strip_diacritics(s).to_lowercase();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Chu cai dau dung cho tieng Viet: "Ánh" -> A, "Đông" -> D, "3M" -> 0-9
pub fn first_letter(name: &str) -> String {
    let s = strip_diacritics(name);
    match s.trim().chars().next() {
        None => "#".to_string(),
        Some(c) => {
            let u = c.to_ascii_uppercase();
            if u.is_ascii_alphabetic() {
                u.to_string()
            } else if u.is_ascii_digit() {
                "0-9".to_string()
            } else {
                "#".to_string()
            }
        }
    }
}

// ------------------------------------------------------------ Ten thu muc an toan

static RESERVED: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .into_iter()
    .collect()
});

/// Lam sach mot doan ten thu muc do phan mem sinh ra.
/// BAT BUOC goi truoc khi ghep vao duong dan dich.
pub fn sanitize_segment(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 || c as u32 == 0x7f => ' ',
            c => c,
        })
        .collect();

    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Windows khong cho ten ket thuc bang dau cham hoac khoang trang
    s = s.trim_end_matches(['.', ' ']).to_string();

    if s.is_empty() {
        return crate::i18n::t("seg.unnamed").to_string();
    }

    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    if RESERVED.contains(stem.as_str()) {
        s.insert(0, '_');
    }

    // Cat ngan de tranh vuot gioi han duong dan
    if s.chars().count() > 110 {
        s = s.chars().take(110).collect::<String>();
        s = s.trim_end_matches(['.', ' ']).to_string();
        if s.is_empty() {
            return crate::i18n::t("seg.unnamed").to_string();
        }
    }
    s
}

/// Ten thu muc dang gach noi: "Báo cáo tháng" -> "Báo-cáo-tháng"
pub fn kebab(raw: &str) -> String {
    let joined = raw.split_whitespace().collect::<Vec<_>>().join("-");
    let cleaned = sanitize_segment(&joined);
    // gom nhieu gach noi lien tiep
    let mut out = String::with_capacity(cleaned.len());
    let mut prev_dash = false;
    for c in cleaned.chars() {
        if c == '-' {
            if !prev_dash {
                out.push(c);
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        crate::i18n::t("seg.unnamed").to_string()
    } else {
        trimmed
    }
}

// -------------------------------------------------------------------- Duong dan

/// Duong dan vuot gioi han tuyet doi cua Windows (~32k ky tu wide)
pub fn is_path_too_long(p: &Path) -> bool {
    cfg!(windows) && p.as_os_str().len() > 30000
}

/// So sanh duong dan (Windows khong phan biet hoa thuong)
pub fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// `child` co nam ben trong `parent` khong
pub fn is_inside(child: &Path, parent: &Path) -> bool {
    let c = normalize_cmp(child);
    let p = normalize_cmp(parent);
    if c == p {
        return false;
    }
    let p_sep = if p.ends_with(std::path::MAIN_SEPARATOR) {
        p.clone()
    } else {
        format!("{}{}", p, std::path::MAIN_SEPARATOR)
    };
    c.starts_with(&p_sep)
}

fn normalize_cmp(p: &Path) -> String {
    let s = p.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// O dia chua duong dan: "D:\a\b" -> "D"
pub fn drive_of(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(c), Some(':')) => c.to_ascii_uppercase().to_string(),
        _ => String::from("?"),
    }
}

/// Tach ten file thanh (phan goc, duoi) - giu duoi kep nhu ".tar.gz"
pub fn split_name(filename: &str) -> (String, String) {
    let lower = filename.to_lowercase();
    for double in [".tar.gz", ".tar.bz2", ".tar.xz"] {
        if lower.ends_with(double) {
            let cut = filename.len() - double.len();
            return (filename[..cut].to_string(), filename[cut..].to_string());
        }
    }
    match filename.rfind('.') {
        Some(i) if i > 0 => (filename[..i].to_string(), filename[i..].to_string()),
        _ => (filename.to_string(), String::new()),
    }
}

pub fn ext_of(filename: &str) -> String {
    match filename.rfind('.') {
        Some(i) if i > 0 && i + 1 < filename.len() => filename[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

// -------------------------------------------------------------------- Dinh dang

pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{} B", n);
    }
    let units = ["KB", "MB", "GB", "TB", "PB"];
    let mut x = n as f64 / 1024.0;
    let mut i = 0usize;
    while x >= 1024.0 && i < units.len() - 1 {
        x /= 1024.0;
        i += 1;
    }
    if x >= 100.0 {
        format!("{:.0} {}", x, units[i])
    } else if x >= 10.0 {
        format!("{:.1} {}", x, units[i])
    } else {
        format!("{:.2} {}", x, units[i])
    }
}

pub fn to_local(ms: i64) -> DateTime<Local> {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(0, 0).single().unwrap())
}

/// Dinh dang thoi gian kieu strftime rut gon.
/// Ho tro: %Y %y %m %d %H %M %B(ten thang) %q(quy) %W(tuan ISO)
/// Dau "/" trong chuoi dinh dang se duoc planner tach thanh nhieu tang.
pub fn strftime(ms: i64, fmt: &str) -> String {
    if ms <= 0 {
        return crate::i18n::t("seg.nodate").to_string();
    }
    let d = to_local(ms);
    let mut out = String::with_capacity(fmt.len() + 8);
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&d.year().to_string()),
            Some('y') => out.push_str(&format!("{:02}", d.year() % 100)),
            Some('m') => out.push_str(&format!("{:02}", d.month())),
            Some('d') => out.push_str(&format!("{:02}", d.day())),
            Some('H') => out.push_str(&format!("{:02}", d.hour_val())),
            Some('M') => out.push_str(&format!("{:02}", d.minute_val())),
            Some('B') => out.push_str(crate::i18n::month(d.month0() as usize)),
            Some('q') => out.push_str(&format!("Q{}", (d.month0() / 3) + 1)),
            Some('W') => out.push_str(&format!("W{:02}", d.iso_week().week())),
            Some('%') => out.push('%'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Rut gon truy cap gio/phut cho DateTime<Local>
trait TimeParts {
    fn hour_val(&self) -> u32;
    fn minute_val(&self) -> u32;
}
impl TimeParts for DateTime<Local> {
    fn hour_val(&self) -> u32 {
        use chrono::Timelike;
        self.hour()
    }
    fn minute_val(&self) -> u32 {
        use chrono::Timelike;
        self.minute()
    }
}

pub fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

/// Ghep cac doan da lam sach thanh duong dan dich
pub fn join_segments(base: &Path, segments: &[String]) -> PathBuf {
    let mut p = base.to_path_buf();
    for s in segments {
        if s.is_empty() {
            continue;
        }
        p.push(s);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diacritics() {
        assert_eq!(strip_diacritics("Báo cáo Đông"), "Bao cao Dong");
        assert_eq!(first_letter("Ánh"), "A");
        assert_eq!(first_letter("Đông"), "D");
        assert_eq!(first_letter("3M"), "0-9");
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_segment("a/b:c"), "a-b-c");
        assert_eq!(sanitize_segment("CON"), "_CON");
        assert_eq!(sanitize_segment("aux.txt"), "_aux.txt");
        assert_eq!(sanitize_segment("ten. "), "ten");
        assert_eq!(sanitize_segment("   "), "_Khong-Ten");
        assert_eq!(sanitize_segment(""), "_Khong-Ten");
    }

    #[test]
    fn names() {
        assert_eq!(split_name("a.tar.gz"), ("a".into(), ".tar.gz".into()));
        assert_eq!(split_name("bao cao.pdf"), ("bao cao".into(), ".pdf".into()));
        assert_eq!(split_name(".gitignore"), (".gitignore".into(), "".into()));
        assert_eq!(ext_of("x.PDF"), "pdf");
    }

    #[test]
    fn inside() {
        assert!(is_inside(Path::new("C:\\a\\b"), Path::new("C:\\a")));
        assert!(!is_inside(Path::new("C:\\a"), Path::new("C:\\a")));
        assert!(!is_inside(Path::new("C:\\ab"), Path::new("C:\\a")));
    }
}
