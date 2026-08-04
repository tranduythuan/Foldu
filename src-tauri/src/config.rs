//! Cau hinh mac dinh + doc/ghi ho so nguoi dung.
//! Luu tai %APPDATA%\Foldu\

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .unwrap_or_else(|_| PathBuf::from("."))
        });
    base.join("Foldu")
}

pub fn journal_dir() -> PathBuf {
    app_dir().join("journal")
}
pub fn profiles_dir() -> PathBuf {
    app_dir().join("profiles")
}
pub fn settings_file() -> PathBuf {
    app_dir().join("settings.json")
}
pub fn reports_dir() -> PathBuf {
    app_dir().join("reports")
}

pub fn ensure_dirs() {
    for d in [app_dir(), journal_dir(), profiles_dir(), reports_dir()] {
        let _ = fs::create_dir_all(d);
    }
}

// --------------------------------------------------------------- Nhom loai file

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileGroup {
    pub name: String,
    pub exts: Vec<String>,
}

fn g(name: &str, exts: &str) -> FileGroup {
    FileGroup {
        name: name.to_string(),
        exts: exts.split(' ').map(|s| s.to_string()).collect(),
    }
}

pub fn default_groups() -> Vec<FileGroup> {
    use crate::i18n::t;
    vec![
        g(t("grp.images"), "jpg jpeg png gif bmp webp heic heif tiff tif svg ico avif jfif"),
        g(t("grp.raw"), "cr2 cr3 nef arw dng raf orf rw2 pef srw sr2 x3f"),
        g(t("grp.video"), "mp4 mkv avi mov wmv flv webm m4v mpg mpeg 3gp ts m2ts vob"),
        g(t("grp.audio"), "mp3 wav flac aac ogg wma m4a opus aiff amr mid"),
        g(t("grp.docs"), "pdf doc docx txt rtf odt md epub pages djvu tex"),
        g(t("grp.sheets"), "xls xlsx csv ods numbers tsv xlsm"),
        g(t("grp.slides"), "ppt pptx odp key pps ppsx"),
        g(t("grp.archives"), "zip rar 7z tar gz bz2 xz iso tgz cab"),
        g(t("grp.installers"), "exe msi msix appx bat cmd ps1 vbs jar apk"),
        g(t("grp.design"), "psd ai indd sketch fig xd afdesign afphoto cdr eps"),
        g(
            t("grp.code"),
            "py js ts jsx tsx java c h cpp hpp cs go rs php rb sql json xml yaml yml html css scss sh",
        ),
        g(t("grp.cad"), "dwg dxf stl obj fbx blend 3ds skp step stp iges gltf glb"),
        g(t("grp.fonts"), "ttf otf woff woff2 eot fon"),
    ]
}

/// Nhóm cho file không khớp nhóm nào. Phụ thuộc ngôn ngữ nên phải là hàm.
pub fn fallback_group() -> &'static str {
    crate::i18n::t("grp.other")
}

// ----------------------------------------------------------- Tu khoa nghiep vu

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordRule {
    pub folder: String,
    pub words: Vec<String>,
}

fn k(folder: &str, words: &[&str]) -> KeywordRule {
    KeywordRule {
        folder: folder.to_string(),
        words: words.iter().map(|s| s.to_string()).collect(),
    }
}

/// Từ khoá giữ cả tiếng Việt lẫn tiếng Anh trong mọi ngôn ngữ, vì một thư mục
/// thật thường lẫn lộn cả hai. Chỉ tên thư mục đích là đổi theo ngôn ngữ.
pub fn default_keywords() -> Vec<KeywordRule> {
    use crate::i18n::t;
    vec![
        k(t("kw.contracts"), &["hop dong", "contract", "agreement", "thoa thuan"]),
        k(t("kw.invoices"), &["hoa don", "invoice", "vat", "bill", "receipt", "bien lai"]),
        k(t("kw.reports"), &["bao cao", "report", "tong ket", "summary", "thong ke"]),
        k(t("kw.hr"), &["cv", "resume", "ho so", "don xin viec", "bang cap", "so yeu ly lich"]),
        k(t("kw.plans"), &["ke hoach", "plan", "roadmap", "chien luoc", "proposal", "de xuat"]),
        k(t("kw.quotes"), &["bao gia", "quotation", "quote", "chao gia", "bang gia", "price list"]),
        k(t("kw.legal"), &["quyet dinh", "thong tu", "nghi dinh", "cong van", "giay phep", "policy", "terms"]),
        k(t("kw.finance"), &["ngan sach", "budget", "thu chi", "cong no", "bang luong", "sao ke", "payroll", "statement"]),
    ]
}

// --------------------------------------------- Tu nhieu (loai khoi ten cum du an)

pub fn default_noise() -> Vec<String> {
    "v v1 v2 v3 v4 v5 ver version final draft cuoi cuoicung ban bansao sao copy new moi old cu \
     edit edited sua fix test temp tmp backup bak chinh update updated \
     img image dsc dscn dcim mvi vid video pxl photo pic \
     screenshot screen shot anh chup man hinh capture snap \
     whatsapp zalo messenger download untitled document file \
     the a an of and for to in on cua va cho voi tai"
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub fn default_ignore() -> Vec<String> {
    [
        "desktop.ini",
        "thumbs.db",
        ".ds_store",
        "**/node_modules/**",
        "**/.git/**",
        "**/$recycle.bin/**",
        "**/system volume information/**",
        "**/.svn/**",
        "**/__pycache__/**",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Cac muc nam NGAY GOC O DIA khong bao gio duoc dong toi — ke ca khi nguoi dung
/// bat "hiện file ẩn / file hệ thống". Danh sach nay la VO DIEU KIEN.
/// Dong toi bat ky muc nao trong day deu co the lam hong o dia hoac he dieu hanh.
pub const DRIVE_ROOT_PROTECTED: &[&str] = &[
    // Sieu du lieu cua chinh o dia
    "$recycle.bin",
    "recycler",
    "system volume information",
    "config.msi",
    "found.000",
    "found.001",
    "$avg",
    "$getcurrent",
    "$sysreset",
    "$windows.~bt",
    "$windows.~ws",
    "$winreagent",
    "recovery",
    "msocache",
    "onedrivetemp",
    // File he thong nam o goc
    "pagefile.sys",
    "hiberfil.sys",
    "swapfile.sys",
    "dumpstack.log",
    "dumpstack.log.tmp",
    "bootmgr",
    "bootnxt",
    "bootsect.bak",
    "ntldr",
    "ntdetect.com",
    "boot.ini",
    "autoexec.bat",
    "config.sys",
    "desktop.ini",
    // Phong truong hop o nay tung/dang chua mot ban Windows khac
    "windows",
    "winnt",
    "program files",
    "program files (x86)",
    "programdata",
    "users",
    "documents and settings",
    "perflogs",
    "boot",
    "efi",
    "system.sav",
];

/// Dau hieu nhan biet mot thu muc la "du an" -> khong pha tung ra
pub const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "requirements.txt",
    "pyproject.toml",
    "composer.json",
    "gemfile",
    "dockerfile",
    "makefile",
];
pub const PROJECT_MARKER_EXTS: &[&str] = &["sln", "csproj", "xcodeproj"];

/// Cac nhom duoi file phai nam chung thu muc neu trung ten goc
pub const SIDECAR_GROUPS: &[&[&str]] = &[
    &["cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2", "jpg", "jpeg", "xmp"],
    &["mp4", "mkv", "avi", "mov", "srt", "ass", "vtt", "sub", "idx", "nfo"],
    &["psd", "png", "jpg", "jpeg", "tif", "tiff"],
    &["obj", "mtl", "fbx", "png", "jpg"],
    &["shp", "shx", "dbf", "prj", "cpg", "sbn"],
    &["dwg", "dwl", "dwl2", "bak"],
];

// ------------------------------------------------------------------- Ho so

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    pub include_hidden: bool,
    pub include_system: bool,
    pub skip_cloud_placeholder: bool,
    pub min_size_bytes: u64,
    pub max_size_bytes: u64,
    pub ignore_patterns: Vec<String>,
    pub ext_whitelist: Vec<String>,
    pub ext_blacklist: Vec<String>,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_system: false,
            skip_cloud_placeholder: true,
            min_size_bytes: 0,
            max_size_bytes: 0,
            ignore_patterns: default_ignore(),
            ext_whitelist: vec![],
            ext_blacklist: vec![],
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyOpts {
    pub keep_sidecar_together: bool,
    pub treat_project_folders_as_unit: bool,
    /// Thu muc chua ung dung da cai/portable duoc de nguyen tai cho.
    /// Di chuyen chung se lam hong duong dan trong registry va shortcut.
    #[serde(default = "yes")]
    pub protect_installed_apps: bool,
    /// Sau khi chuyen file ra khoi thu muc con, cac vo thu muc rong con lai
    /// se duoc don di. Khong co buoc nay thi don xong van con mot dong rac.
    #[serde(default = "yes")]
    pub clean_empty_dirs: bool,
    pub max_new_folders: usize,
}

impl Default for SafetyOpts {
    fn default() -> Self {
        Self {
            keep_sidecar_together: true,
            treat_project_folders_as_unit: true,
            protect_installed_apps: true,
            clean_empty_dirs: true,
            max_new_folders: 500,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DupStrategy {
    KeepOldest,
    KeepNewest,
    KeepShortestPath,
    KeepCleanestName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DupAction {
    Quarantine,
    Recycle,
    Report,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupOpts {
    pub enabled: bool,
    pub strategy: DupStrategy,
    pub action: DupAction,
    /// Tim them anh GAN giong nhau, khong chi giong het tung byte
    #[serde(default = "yes")]
    pub near_images: bool,
    /// So bit duoc phep lech giua hai anh. Cang lon cang bat rong, cang de nham.
    #[serde(default = "default_near_threshold")]
    pub near_threshold: u32,
}

fn default_near_threshold() -> u32 {
    10
}

impl Default for DupOpts {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: DupStrategy::KeepShortestPath,
            action: DupAction::Quarantine,
            near_images: true,
            near_threshold: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOpts {
    pub granularity: u32,
    pub max_tokens: usize,
}

impl Default for ClusterOpts {
    fn default() -> Self {
        Self {
            granularity: 50,
            max_tokens: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Move,
    Copy,
    Hardlink,
    ReportOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub mode: Mode,
    /// Vi du: ["TYPE", "TIME_MODIFIED:%Y/%B", "AUTO_PROJECT"]
    pub layers: Vec<String>,
    pub recursive: bool,
    pub destination: Option<String>,
    #[serde(default)]
    pub filters: Filters,
    #[serde(default)]
    pub safety: SafetyOpts,
    #[serde(default)]
    pub duplicates: DupOpts,
    #[serde(default)]
    pub clustering: ClusterOpts,
    #[serde(default)]
    pub number_prefix: bool,
    #[serde(default)]
    pub rename: crate::rename::RenameSpec,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Mac dinh".into(),
            mode: Mode::Move,
            layers: vec!["TYPE".into(), "TIME_MODIFIED:%Y".into()],
            recursive: true,
            destination: None,
            filters: Filters::default(),
            safety: SafetyOpts::default(),
            duplicates: DupOpts::default(),
            clustering: ClusterOpts::default(),
            number_prefix: true,
            rename: crate::rename::RenameSpec::default(),
        }
    }
}

// ------------------------------------------------------------------ Settings

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub groups: Vec<FileGroup>,
    pub keywords: Vec<KeywordRule>,
    pub noise_words: Vec<String>,
    pub theme: String,
    #[serde(default)]
    pub lang: crate::i18n::Lang,
    /// Nguoi dung da tu chon ngon ngu chua. Chua thi man dau tien se hoi.
    #[serde(default)]
    pub lang_picked: bool,
    pub recent_paths: Vec<String>,
    pub last_profile: Profile,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            groups: default_groups(),
            keywords: default_keywords(),
            noise_words: default_noise(),
            theme: "dark".into(),
            lang: crate::i18n::lang(),
            lang_picked: false,
            recent_paths: vec![],
            last_profile: Profile::default(),
        }
    }
}

pub fn load_settings() -> Settings {
    ensure_dirs();
    let raw = fs::read_to_string(settings_file()).ok();

    // Ngon ngu phai duoc dat TRUOC khi dung Default, vi moi gia tri mac dinh
    // (ten nhom file, tu khoa, mau dung san) deu sinh ra theo ngon ngu dang chon.
    match &raw {
        Some(r) => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(r) {
                if let Some(l) = v.get("lang").and_then(|x| x.as_str()) {
                    crate::i18n::set_lang(crate::i18n::Lang::from_code(l));
                }
            }
        }
        // Lan dau chay: doan theo ngon ngu he dieu hanh de goi y san,
        // nhung man dau tien van hoi lai cho nguoi dung tu chon.
        None => crate::i18n::set_lang(crate::i18n::system_lang()),
    }

    raw.and_then(|r| serde_json::from_str::<Settings>(&r).ok())
        .unwrap_or_default()
}

/// Bang nhom file / tu khoa co dang y het bo mac dinh cua mot ngon ngu khong.
/// Dung de biet co the doi sang bo mac dinh cua ngon ngu moi ma khong mat cong sua tay.
pub fn matches_defaults_of(s: &Settings, l: crate::i18n::Lang) -> bool {
    let cur = crate::i18n::lang();
    crate::i18n::set_lang(l);
    let same = {
        let g = default_groups();
        let k = default_keywords();
        s.groups.len() == g.len()
            && s.groups.iter().zip(&g).all(|(a, b)| a.name == b.name && a.exts == b.exts)
            && s.keywords.len() == k.len()
            && s.keywords.iter().zip(&k).all(|(a, b)| a.folder == b.folder)
    };
    crate::i18n::set_lang(cur);
    same
}

pub fn save_settings(s: &Settings) -> Result<(), String> {
    ensure_dirs();
    let tmp = settings_file().with_extension("json.tmp");
    let data = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    fs::rename(&tmp, settings_file()).map_err(|e| e.to_string())?;
    Ok(())
}

// -------------------------------------------------------------- Mau dung san

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    pub layers: Vec<&'static str>,
    pub mode: Mode,
}

pub fn presets() -> Vec<Preset> {
    use crate::i18n::t;
    let p = |id: &'static str, key: &str, icon: &'static str, layers: Vec<&'static str>, mode: Mode| Preset {
        id,
        name: t(&format!("pre.{}.name", key)),
        desc: t(&format!("pre.{}.desc", key)),
        icon,
        layers,
        mode,
    };
    vec![
        p("downloads", "downloads", "download", vec!["TYPE", "TIME_RELATIVE"], Mode::Move),
        p("photos", "photos", "image", vec!["SCREENSHOT_DETECT", "TIME_TAKEN:%Y/%B"], Mode::Move),
        p("projects", "projects", "folders", vec!["AUTO_PROJECT", "TYPE"], Mode::Move),
        p("company", "company", "briefcase",
          vec!["TIME_MODIFIED:%Y", "TIME_MODIFIED:%q", "KEYWORD_RULE"], Mode::Move),
        p("diskfull", "diskfull", "drive", vec!["SIZE_TIER", "TYPE"], Mode::Move),
        p("media", "media", "film", vec!["TYPE", "MEDIA_RESOLUTION", "TIME_MODIFIED:%Y"], Mode::Move),
        p("archive", "archive", "archive", vec!["ACCESS_HEAT", "TIME_MODIFIED:%Y"], Mode::Move),
        // "Chi tim file bi luu trung" tung nam o day, nhung no khong phai mot kieu
        // SAP XEP — no la mot cong viec khac han, gio co man hinh rieng "Tim file &
        // anh trung". Bo khoi day cung lam man chon kieu nhe di mot the.
    ]
}
