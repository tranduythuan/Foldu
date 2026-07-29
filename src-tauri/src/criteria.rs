//! 24 tieu chi sap xep, chia 5 nhom.
//! Moi tieu chi nhan mot file + ngu canh, tra ve cac DOAN duong dan (co the rong).
//! Doan rong = tang do khong sinh thu muc cho file nay.

use crate::config::{fallback_group, Settings};
use crate::i18n::{t, tf};
use crate::media::MediaInfo;
use crate::scanner::FileEntry;
use crate::util::{
    first_letter, norm_key, sanitize_segment, split_name, strftime, strip_diacritics,
};
use std::collections::HashMap;

pub const DAY_MS: i64 = 86_400_000;

// -------------------------------------------------------------- Ngu canh

pub struct Ctx<'a> {
    pub settings: &'a Settings,
    pub now_ms: i64,
    /// AUTO_PROJECT: id file -> ten cum
    pub clusters: HashMap<u32, String>,
    /// VERSION_GROUP: id file -> ten nhom phien ban
    pub versions: HashMap<u32, String>,
    /// Metadata media doc lazy: id file -> thong tin
    pub media: HashMap<u32, MediaInfo>,
    /// DOWNLOAD_SOURCE: id file -> ten mien
    pub sources: HashMap<u32, String>,
    /// Nguong kich thuoc theo phan vi cua chinh tap file dang quet
    pub size_p: [u64; 4],
    pub number_prefix: bool,
}

// -------------------------------------------------------- Phan tich chuoi tieu chi

/// "TIME_MODIFIED:%Y/%B" -> ("TIME_MODIFIED", "%Y/%B")
pub fn parse_layer(spec: &str) -> (&str, &str) {
    match spec.find(':') {
        Some(i) => (&spec[..i], &spec[i + 1..]),
        None => (spec, ""),
    }
}

/// Tieu chi nao can doc noi dung file (EXIF, magic bytes, kich thuoc anh)
pub fn needs_media(layers: &[String]) -> bool {
    layers.iter().any(|l| {
        matches!(
            parse_layer(l).0,
            "REAL_TYPE" | "TIME_TAKEN" | "MEDIA_RESOLUTION" | "IMAGE_ORIENTATION"
                | "EXIF_CAMERA" | "SCREENSHOT_DETECT" | "EXIF_GPS_PLACE"
        )
    })
}

pub fn needs_download_source(layers: &[String]) -> bool {
    layers.iter().any(|l| parse_layer(l).0 == "DOWNLOAD_SOURCE")
}

pub fn needs_clustering(layers: &[String]) -> bool {
    layers.iter().any(|l| parse_layer(l).0 == "AUTO_PROJECT")
}

pub fn needs_versions(layers: &[String]) -> bool {
    layers.iter().any(|l| parse_layer(l).0 == "VERSION_GROUP")
}

// ------------------------------------------------------------------ Bac kich thuoc

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

fn size_tier_fixed(size: u64) -> &'static str {
    if size >= GB {
        t("seg.size.huge")
    } else if size >= 100 * MB {
        t("seg.size.large")
    } else if size >= 10 * MB {
        t("seg.size.medium")
    } else if size >= MB {
        t("seg.size.small")
    } else {
        t("seg.size.tiny")
    }
}

fn size_tier_auto(size: u64, p: &[u64; 4]) -> String {
    // p = [p25, p50, p75, p90] cua chinh tap file dang quet
    if size >= p[3] {
        tf("seg.auto.p90", &[&crate::util::format_bytes(p[3])])
    } else if size >= p[2] {
        tf("seg.auto.p75", &[&crate::util::format_bytes(p[2])])
    } else if size >= p[1] {
        tf("seg.auto.p50", &[&crate::util::format_bytes(p[1])])
    } else if size >= p[0] {
        tf("seg.auto.p25", &[&crate::util::format_bytes(p[0])])
    } else {
        t("seg.auto.rest").to_string()
    }
}

pub fn percentiles(sizes: &mut [u64]) -> [u64; 4] {
    if sizes.is_empty() {
        return [0, 0, 0, 0];
    }
    sizes.sort_unstable();
    let at = |q: f64| -> u64 {
        let i = ((sizes.len() - 1) as f64 * q).round() as usize;
        sizes[i.min(sizes.len() - 1)]
    };
    [at(0.25), at(0.50), at(0.75), at(0.90)]
}

// ------------------------------------------------------------------ Thoi gian

fn time_relative(ms: i64, now: i64) -> &'static str {
    if ms <= 0 {
        return t("seg.nodate");
    }
    let age = now - ms;
    if age < DAY_MS {
        t("seg.rel.today")
    } else if age < 7 * DAY_MS {
        t("seg.rel.week")
    } else if age < 30 * DAY_MS {
        t("seg.rel.month")
    } else if age < 90 * DAY_MS {
        t("seg.rel.quarter")
    } else if age < 365 * DAY_MS {
        t("seg.rel.year")
    } else {
        t("seg.rel.older")
    }
}

fn access_heat(atime: i64, mtime: i64, now: i64) -> &'static str {
    // Mot so he thong tat cap nhat atime -> dung mtime lam du phong
    let last = if atime > 0 { atime.max(mtime) } else { mtime };
    if last <= 0 {
        return t("seg.unknown");
    }
    let age = now - last;
    if age < 30 * DAY_MS {
        t("seg.heat.hot")
    } else if age < 180 * DAY_MS {
        t("seg.heat.warm")
    } else if age < 365 * DAY_MS {
        t("seg.heat.cool")
    } else {
        t("seg.heat.frozen")
    }
}

// --------------------------------------------------------------- Nhom loai file

fn type_group(ext: &str, s: &Settings, number_prefix: bool) -> String {
    let name = s
        .groups
        .iter()
        .find(|g| g.exts.iter().any(|e| e == ext))
        .map(|g| g.name.clone())
        .unwrap_or_else(|| fallback_group().to_string());
    if number_prefix {
        name
    } else {
        strip_number_prefix(&name)
    }
}

fn strip_number_prefix(s: &str) -> String {
    let b = s.as_bytes();
    if b.len() > 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'-' {
        s[3..].to_string()
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------- Do phan giai

fn resolution_tier(w: u32, h: u32) -> &'static str {
    let long = w.max(h);
    if long >= 7680 {
        t("seg.res.8k")
    } else if long >= 3840 {
        t("seg.res.4k")
    } else if long >= 2560 {
        t("seg.res.2k")
    } else if long >= 1920 {
        t("seg.res.1080")
    } else if long >= 1280 {
        t("seg.res.720")
    } else {
        t("seg.res.low")
    }
}

fn orientation(w: u32, h: u32) -> &'static str {
    if w == 0 || h == 0 {
        return t("seg.unknown");
    }
    let r = w as f64 / h as f64;
    if r > 1.05 {
        t("seg.orient.land")
    } else if r < 0.95 {
        t("seg.orient.port")
    } else {
        t("seg.orient.sq")
    }
}

// ------------------------------------------------------------- Anh chup man hinh

const SCREENSHOT_HINTS: &[&str] = &[
    "screenshot", "screen shot", "screen_shot", "screencapture", "screen capture",
    "anh chup man hinh", "chup man hinh", "man hinh", "scr_", "snipaste", "lightshot",
];

fn is_screenshot(f: &FileEntry, m: Option<&MediaInfo>) -> bool {
    let n = norm_key(&f.name);
    if SCREENSHOT_HINTS.iter().any(|h| n.contains(h)) {
        return true;
    }
    // Anh PNG khong co thong tin may anh -> rat co kha nang la anh chup man hinh
    match m {
        Some(mi) if mi.real_kind.as_deref() == Some("image") => {
            mi.real_ext.as_deref() == Some("png") && mi.camera.is_none() && mi.taken_ms.is_none()
        }
        _ => false,
    }
}

// -------------------------------------------------------------- Ngon ngu ten file

fn language_script(name: &str) -> &'static str {
    let mut has_cjk = false;
    let mut has_kana = false;
    let mut has_hangul = false;
    let mut has_cyr = false;
    for c in name.chars() {
        let u = c as u32;
        match u {
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => has_cjk = true,
            0x3040..=0x30FF => has_kana = true,
            0xAC00..=0xD7AF => has_hangul = true,
            0x0400..=0x04FF => has_cyr = true,
            _ => {}
        }
    }
    if has_kana {
        return t("seg.script.ja");
    }
    if has_hangul {
        return t("seg.script.ko");
    }
    if has_cjk {
        return t("seg.script.zh");
    }
    if has_cyr {
        return t("seg.script.ru");
    }
    // Co dau tieng Viet?
    if strip_diacritics(name) != name {
        return t("seg.script.vi");
    }
    t("seg.script.latin")
}

// ---------------------------------------------------------------- Nhom phien ban

const VERSION_RE_TOKENS: &[&str] = &[
    "final", "cuoi", "cuoicung", "draft", "nhap", "copy", "bansao", "sao", "new", "moi",
    "old", "cu", "edit", "edited", "sua", "fix", "update", "updated", "revised", "rev",
];

/// Bo cac dau hieu phien ban khoi ten de lay "ten goc"
fn version_base(stem: &str) -> String {
    let cleaned = strip_diacritics(stem).to_lowercase();
    let toks: Vec<String> = cleaned
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| {
            if VERSION_RE_TOKENS.contains(t) {
                return false;
            }
            // v1, v2, ver3, r2 ...
            let b = t.as_bytes();
            if (b[0] == b'v' || b[0] == b'r') && b[1..].iter().all(|c| c.is_ascii_digit()) && b.len() > 1 {
                return false;
            }
            if *t == "v" || *t == "ver" || *t == "version" {
                return false;
            }
            // so thu tu don le o cuoi: "(1)", "2"
            if t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 2 {
                return false;
            }
            true
        })
        .map(|t| t.to_string())
        .collect();
    toks.join(" ")
}

/// Tien xu ly: gom cac file la phien ban cua nhau.
/// Chi nhung nhom co >= 2 file moi duoc coi la nhom phien ban.
pub fn build_version_groups(files: &[FileEntry]) -> HashMap<u32, String> {
    let mut by_base: HashMap<String, Vec<(u32, String)>> = HashMap::new();
    for f in files {
        if f.is_dir {
            continue;
        }
        let (stem, _) = split_name(&f.name);
        let base = version_base(&stem);
        if base.is_empty() {
            continue;
        }
        by_base.entry(base).or_default().push((f.id, stem));
    }
    let mut out = HashMap::new();
    for (_base, members) in by_base {
        if members.len() < 2 {
            continue;
        }
        // Ten thu muc lay tu ban co ten NGAN nhat (thuong la ban goc, sach nhat)
        let label = members
            .iter()
            .map(|(_, s)| s.as_str())
            .min_by_key(|s| s.chars().count())
            .unwrap_or("")
            .to_string();
        let folder = crate::util::kebab(&label);
        for (id, _) in members {
            out.insert(id, folder.clone());
        }
    }
    out
}

// ------------------------------------------------------------------ Ham chinh

/// Sinh cac doan duong dan cho MOT tang tieu chi.
/// Tra ve vec rong = tang nay khong sinh thu muc cho file nay.
pub fn segments(spec: &str, f: &FileEntry, ctx: &Ctx) -> Vec<String> {
    let (id, arg) = parse_layer(spec);
    let m = ctx.media.get(&f.id);

    let raw: Vec<String> = match id {
        "NONE" | "" => vec![],

        "LITERAL" => arg.split('/').map(|s| s.to_string()).collect(),

        // ------------------------------------------------- Nhom A: co ban
        "TYPE" => {
            if f.is_dir {
                vec![t("seg.project").to_string()]
            } else {
                vec![type_group(&f.ext, ctx.settings, ctx.number_prefix)]
            }
        }
        "EXT" => vec![if f.is_dir {
            t("seg.folder").to_string()
        } else if f.ext.is_empty() {
            t("seg.noext").to_string()
        } else {
            f.ext.to_uppercase()
        }],
        "REAL_TYPE" => {
            let real = m.and_then(|x| x.real_ext.clone());
            match real {
                Some(r) if !f.ext.is_empty() && r != f.ext && !same_family(&r, &f.ext) => {
                    vec![t("seg.wrongext").to_string(), r.to_uppercase()]
                }
                Some(r) => vec![r.to_uppercase()],
                None => vec![if f.ext.is_empty() {
                    t("seg.unknown").to_string()
                } else {
                    f.ext.to_uppercase()
                }],
            }
        }
        "SIZE_TIER" => vec![size_tier_fixed(f.size).to_string()],
        "SIZE_TIER_AUTO" => vec![size_tier_auto(f.size, &ctx.size_p)],
        "ALPHABET" => vec![first_letter(&f.name)],

        // ------------------------------------------------ Nhom B: thoi gian
        "TIME_MODIFIED" => split_fmt(strftime(f.mtime, fmt_or(arg, "%Y/%m"))),
        "TIME_CREATED" => split_fmt(strftime(f.ctime, fmt_or(arg, "%Y/%m"))),
        "TIME_TAKEN" => {
            let t = m.and_then(|x| x.taken_ms).unwrap_or(f.mtime);
            split_fmt(strftime(t, fmt_or(arg, "%Y/%B")))
        }
        "TIME_RELATIVE" => vec![time_relative(f.mtime, ctx.now_ms).to_string()],
        "TIME_QUARTER" => split_fmt(strftime(f.mtime, fmt_or(arg, "%Y-%q"))),
        "TIME_WEEK" => split_fmt(strftime(f.mtime, fmt_or(arg, "%Y-%W"))),
        "ACCESS_HEAT" => vec![access_heat(f.atime, f.mtime, ctx.now_ms).to_string()],

        // --------------------------------------- Nhom C: noi dung & metadata
        "MEDIA_RESOLUTION" => match m.and_then(|x| dims(x)) {
            Some((w, h)) => vec![resolution_tier(w, h).to_string()],
            None => vec![],
        },
        "IMAGE_ORIENTATION" => match m.and_then(|x| dims(x)) {
            Some((w, h)) => vec![orientation(w, h).to_string()],
            None => vec![],
        },
        "EXIF_CAMERA" => match m.and_then(|x| x.camera.clone()) {
            Some(c) => vec![crate::util::kebab(&c)],
            None => vec![],
        },
        "EXIF_GPS_PLACE" => match m.and_then(|x| x.place.clone()) {
            Some(p) => vec![p],
            // Ảnh không có GPS (chụp màn hình, tải về, đã xoá định vị) -> gom riêng
            None => vec![t("seg.noplace").to_string()],
        },
        "SCREENSHOT_DETECT" => {
            if is_screenshot(f, m) {
                vec![t("seg.screenshot").to_string()]
            } else {
                vec![]
            }
        }

        // ------------------------------------- Nhom D: ngu nghia & quan he
        "AUTO_PROJECT" => vec![ctx
            .clusters
            .get(&f.id)
            .cloned()
            .unwrap_or_else(|| crate::clustering::other().to_string())],
        "VERSION_GROUP" => match ctx.versions.get(&f.id) {
            Some(v) => vec![v.clone()],
            None => vec![],
        },
        "KEYWORD_RULE" => {
            let n = norm_key(&f.name);
            match ctx
                .settings
                .keywords
                .iter()
                .find(|r| r.words.iter().any(|w| n.contains(&norm_key(w))))
            {
                Some(r) => vec![r.folder.clone()],
                None => vec![t("seg.other").to_string()],
            }
        }
        "DOWNLOAD_SOURCE" => match ctx.sources.get(&f.id) {
            Some(d) => vec![tf("seg.downloaded", &[d])],
            None => vec![t("seg.nosource").to_string()],
        },
        "LANGUAGE_SCRIPT" => vec![language_script(&f.name).to_string()],

        // ------------------------------------------------- Nhom E: he thong
        "ORIGIN_FOLDER" => {
            let p = f
                .parent
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if p.is_empty() {
                vec![]
            } else {
                vec![tf("seg.from", &[&crate::util::kebab(&p)])]
            }
        }

        _ => vec![],
    };

    raw.into_iter()
        .map(|s| sanitize_segment(&s))
        .filter(|s| !s.is_empty())
        .collect()
}

fn dims(m: &MediaInfo) -> Option<(u32, u32)> {
    match (m.width, m.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
        _ => None,
    }
}

fn fmt_or<'a>(arg: &'a str, default: &'a str) -> &'a str {
    if arg.is_empty() {
        default
    } else {
        arg
    }
}

/// "%Y/%B" -> ["2026", "03-Thang-Ba"]
fn split_fmt(s: String) -> Vec<String> {
    s.split(['/', '\\'])
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .collect()
}

/// jpg/jpeg, tif/tiff... khong tinh la "sai duoi file"
fn same_family(a: &str, b: &str) -> bool {
    const FAM: &[&[&str]] = &[
        &["jpg", "jpeg", "jfif"],
        &["tif", "tiff"],
        &["htm", "html"],
        &["zip", "docx", "xlsx", "pptx", "apk", "jar", "epub"], // deu la container ZIP
        &["mp4", "m4v", "m4a", "mov"],
        &["exe", "dll", "msi", "sys"],
        &["doc", "xls", "ppt", "msg"], // deu la container OLE
    ];
    FAM.iter().any(|f| f.contains(&a) && f.contains(&b))
}

/// Danh sach tieu chi hien ra giao dien
pub fn catalog() -> serde_json::Value {
    // Danh muc dung chung cho moi ngon ngu; chu nghia lay tu bang i18n.
    fn item(id: &str, arg: Option<&str>, slow: bool) -> serde_json::Value {
        let mut o = serde_json::json!({
            "id": id,
            "name": t(&format!("cat.{}.n", id)),
            "desc": t(&format!("cat.{}.d", id)),
        });
        if let Some(a) = arg {
            o["arg"] = serde_json::Value::String(a.to_string());
        }
        if slow {
            o["slow"] = serde_json::Value::Bool(true);
        }
        o
    }
    fn group(key: &str, items: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "group": t(key), "items": items })
    }

    serde_json::json!([
        group("cat.g.common", vec![
            item("TYPE", None, false),
            item("AUTO_PROJECT", None, false),
            item("TIME_MODIFIED", Some("%Y/%m"), false),
            item("TIME_RELATIVE", None, false),
            item("SIZE_TIER", None, false),
            item("ALPHABET", None, false),
        ]),
        group("cat.g.photo", vec![
            item("TIME_TAKEN", Some("%Y/%B"), true),
            item("EXIF_GPS_PLACE", None, true),
            item("SCREENSHOT_DETECT", None, true),
            item("EXIF_CAMERA", None, true),
            item("MEDIA_RESOLUTION", None, true),
            item("IMAGE_ORIENTATION", None, true),
        ]),
        group("cat.g.work", vec![
            item("KEYWORD_RULE", None, false),
            item("VERSION_GROUP", None, false),
            item("TIME_CREATED", Some("%Y/%m"), false),
            item("ACCESS_HEAT", None, false),
        ]),
        group("cat.g.rare", vec![
            item("EXT", None, false),
            item("SIZE_TIER_AUTO", None, false),
            item("REAL_TYPE", None, true),
            item("DOWNLOAD_SOURCE", None, false),
            item("TIME_QUARTER", Some("%Y-%q"), false),
            item("TIME_WEEK", Some("%Y-%W"), false),
            item("LANGUAGE_SCRIPT", None, false),
            item("ORIGIN_FOLDER", None, false),
            item("LITERAL", Some("Ten-Thu-Muc"), false),
        ]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::i18n::set_lang(crate::i18n::Lang::Vi);

        assert_eq!(size_tier_fixed(2 * GB), "01-Rat-Lon-tren-1GB");
        assert_eq!(size_tier_fixed(500), "05-Rat-Nho-duoi-1MB");
    }

    #[test]
    fn percentile_calc() {
        let mut v = vec![1u64, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let p = percentiles(&mut v);
        assert!(p[0] <= p[1] && p[1] <= p[2] && p[2] <= p[3]);
    }

    #[test]
    fn scripts() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::i18n::set_lang(crate::i18n::Lang::Vi);

        assert_eq!(language_script("Báo cáo.pdf"), "Tieng-Viet");
        assert_eq!(language_script("report.pdf"), "Latin");
        assert_eq!(language_script("報告.pdf"), "Tieng-Trung");
        assert_eq!(language_script("レポート.pdf"), "Tieng-Nhat");
    }

    #[test]
    fn version_base_strips_markers() {
        assert_eq!(version_base("Hop dong v2"), "hop dong");
        assert_eq!(version_base("Hop dong final"), "hop dong");
        assert_eq!(version_base("Hop dong - Copy"), "hop dong");
    }

    #[test]
    fn family_not_flagged() {
        assert!(same_family("jpg", "jpeg"));
        assert!(same_family("docx", "zip"));
        assert!(!same_family("exe", "jpg"));
    }

    #[test]
    fn split_format_creates_nesting() {
        assert_eq!(split_fmt("2026/03".into()), vec!["2026", "03"]);
    }
}
