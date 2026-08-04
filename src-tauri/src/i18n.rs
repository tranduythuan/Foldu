//! Đa ngôn ngữ cho lớp lõi.
//!
//! Hai nhóm chuỗi rất khác nhau về hậu quả:
//!
//!   * `seg.*` — TÊN THƯ MỤC được tạo ra trên ổ đĩa. Đổi ngôn ngữ là đổi luôn
//!     cấu trúc thư mục sinh ra. Vì vậy chỉ dùng chữ ASCII không dấu, hợp lệ trên
//!     mọi hệ thống tập tin, và tuyệt đối không chứa ký tự Windows cấm.
//!   * còn lại — chữ hiện trên màn hình, đổi thoải mái.
//!
//! Ngôn ngữ là trạng thái toàn cục vì ứng dụng desktop chỉ có một người dùng và
//! một ngôn ngữ tại một thời điểm. Truyền tham số `lang` xuyên qua mọi hàm sẽ làm
//! rối lớp lõi mà không đem lại lợi ích gì.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Lang {
    #[default]
    #[serde(rename = "vi")]
    Vi,
    #[serde(rename = "en")]
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Vi => "vi",
            Lang::En => "en",
        }
    }
    pub fn from_code(s: &str) -> Lang {
        match s {
            "en" => Lang::En,
            _ => Lang::Vi,
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(l: Lang) {
    LANG.store(if l == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::En
    } else {
        Lang::Vi
    }
}

/// Ngôn ngữ của hệ điều hành, dùng để đoán sẵn lựa chọn ở lần mở đầu tiên.
/// Người dùng vẫn được hỏi và quyết định, đây chỉ là gợi ý.
#[cfg(windows)]
pub fn system_lang() -> Lang {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(lp_locale_name: *mut u16, cch_locale_name: i32) -> i32;
    }
    let mut buf = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 1 {
        return Lang::En;
    }
    let name = String::from_utf16_lossy(&buf[..(n as usize - 1)]).to_lowercase();
    if name.starts_with("vi") {
        Lang::Vi
    } else {
        Lang::En
    }
}

#[cfg(not(windows))]
pub fn system_lang() -> Lang {
    match std::env::var("LANG").unwrap_or_default().to_lowercase() {
        s if s.starts_with("vi") => Lang::Vi,
        _ => Lang::En,
    }
}

/// Tra chuỗi theo ngôn ngữ đang chọn.
pub fn t(key: &str) -> &'static str {
    let (vi, en) = pair(key);
    match lang() {
        Lang::Vi => vi,
        Lang::En => en,
    }
}

/// Như `t` nhưng thay lần lượt các dấu `{}` bằng tham số.
pub fn tf(key: &str, args: &[&str]) -> String {
    let mut out = t(key).to_string();
    for a in args {
        if let Some(i) = out.find("{}") {
            out.replace_range(i..i + 2, a);
        }
    }
    out
}

/// Tên tháng dùng trong tên thư mục. Chỉ ASCII.
pub fn month(m0: usize) -> &'static str {
    const VI: [&str; 12] = [
        "01-Thang-Mot", "02-Thang-Hai", "03-Thang-Ba", "04-Thang-Tu",
        "05-Thang-Nam", "06-Thang-Sau", "07-Thang-Bay", "08-Thang-Tam",
        "09-Thang-Chin", "10-Thang-Muoi", "11-Thang-Muoi-Mot", "12-Thang-Muoi-Hai",
    ];
    const EN: [&str; 12] = [
        "01-January", "02-February", "03-March", "04-April",
        "05-May", "06-June", "07-July", "08-August",
        "09-September", "10-October", "11-November", "12-December",
    ];
    let i = m0.min(11);
    match lang() {
        Lang::Vi => VI[i],
        Lang::En => EN[i],
    }
}

#[rustfmt::skip]
fn pair(key: &str) -> (&'static str, &'static str) {
    match key {

    // ══════════════════════════ TÊN THƯ MỤC TẠO RA TRÊN Ổ ĐĨA (chỉ ASCII) ═══

    // Nhóm loại file
    "grp.images"      => ("01-Hinh-Anh",    "01-Images"),
    "grp.raw"         => ("02-Anh-RAW",     "02-RAW-Photos"),
    "grp.video"       => ("03-Video",       "03-Video"),
    "grp.audio"       => ("04-Am-Thanh",    "04-Audio"),
    "grp.docs"        => ("05-Tai-Lieu",    "05-Documents"),
    "grp.sheets"      => ("06-Bang-Tinh",   "06-Spreadsheets"),
    "grp.slides"      => ("07-Trinh-Chieu", "07-Presentations"),
    "grp.archives"    => ("08-Nen",         "08-Archives"),
    "grp.installers"  => ("09-Cai-Dat",     "09-Installers"),
    "grp.design"      => ("10-Thiet-Ke",    "10-Design"),
    "grp.code"        => ("11-Lap-Trinh",   "11-Code"),
    "grp.cad"         => ("12-3D-CAD",      "12-3D-CAD"),
    "grp.fonts"       => ("13-Font",        "13-Fonts"),
    "grp.other"       => ("14-Khac",        "14-Other"),

    // Thư mục từ khoá nghiệp vụ
    "kw.contracts"    => ("Hop-Dong",        "Contracts"),
    "kw.invoices"     => ("Hoa-Don",         "Invoices"),
    "kw.reports"      => ("Bao-Cao",         "Reports"),
    "kw.hr"           => ("Ho-So-Nhan-Su",   "HR-Records"),
    "kw.plans"        => ("Ke-Hoach",        "Plans"),
    "kw.quotes"       => ("Bao-Gia",         "Quotes"),
    "kw.legal"        => ("Van-Ban-Phap-Ly", "Legal-Documents"),
    "kw.finance"      => ("Tai-Chinh",       "Finance"),

    // Bậc kích thước cố định
    "seg.size.huge"   => ("01-Rat-Lon-tren-1GB",   "01-Huge-over-1GB"),
    "seg.size.large"  => ("02-Lon-100MB-1GB",      "02-Large-100MB-1GB"),
    "seg.size.medium" => ("03-Vua-10-100MB",       "03-Medium-10-100MB"),
    "seg.size.small"  => ("04-Nho-1-10MB",         "04-Small-1-10MB"),
    "seg.size.tiny"   => ("05-Rat-Nho-duoi-1MB",   "05-Tiny-under-1MB"),

    // Bậc kích thước động (so với chính tập file đang quét)
    "seg.auto.p90"    => ("01-Nang-nhat-tren-{}",   "01-Heaviest-over-{}"),
    "seg.auto.p75"    => ("02-Nang-tren-{}",        "02-Heavy-over-{}"),
    "seg.auto.p50"    => ("03-Trung-binh-tren-{}",  "03-Average-over-{}"),
    "seg.auto.p25"    => ("04-Nhe-tren-{}",         "04-Light-over-{}"),
    "seg.auto.rest"   => ("05-Nhe-nhat",            "05-Lightest"),

    // Độ mới tương đối
    "seg.rel.today"   => ("01-Hom-Nay",       "01-Today"),
    "seg.rel.week"    => ("02-7-Ngay-Qua",    "02-Past-7-Days"),
    "seg.rel.month"   => ("03-30-Ngay-Qua",   "03-Past-30-Days"),
    "seg.rel.quarter" => ("04-3-Thang-Qua",   "04-Past-3-Months"),
    "seg.rel.year"    => ("05-Nam-Nay",       "05-This-Year"),
    "seg.rel.older"   => ("06-Cu-Hon-1-Nam",  "06-Over-1-Year-Old"),

    // Độ nguội truy cập
    "seg.heat.hot"    => ("01-Nong-duoi-30-ngay",    "01-Hot-under-30-days"),
    "seg.heat.warm"   => ("02-Am-duoi-6-thang",      "02-Warm-under-6-months"),
    "seg.heat.cool"   => ("03-Nguoi-duoi-1-nam",     "03-Cool-under-1-year"),
    "seg.heat.frozen" => ("04-Dong-Bang-tren-1-nam", "04-Frozen-over-1-year"),

    // Độ phân giải
    "seg.res.8k"      => ("01-8K",                  "01-8K"),
    "seg.res.4k"      => ("02-4K",                  "02-4K"),
    "seg.res.2k"      => ("03-2K-1440p",            "03-2K-1440p"),
    "seg.res.1080"    => ("04-1080p",               "04-1080p"),
    "seg.res.720"     => ("05-720p",                "05-720p"),
    "seg.res.low"     => ("06-Do-phan-giai-thap",   "06-Low-Resolution"),

    // Hướng ảnh
    "seg.orient.land" => ("Ngang",  "Landscape"),
    "seg.orient.port" => ("Doc",    "Portrait"),
    "seg.orient.sq"   => ("Vuong",  "Square"),

    // Ngôn ngữ tên file
    "seg.script.ja"   => ("Tieng-Nhat",  "Japanese"),
    "seg.script.ko"   => ("Tieng-Han",   "Korean"),
    "seg.script.zh"   => ("Tieng-Trung", "Chinese"),
    "seg.script.ru"   => ("Tieng-Nga",   "Russian"),
    "seg.script.vi"   => ("Tieng-Viet",  "Vietnamese"),
    "seg.script.latin"=> ("Latin",       "Latin"),

    // Các đoạn khác
    "seg.project"     => ("00-Du-An",            "00-Projects"),
    "seg.folder"      => ("Thu-Muc",             "Folder"),
    "seg.noext"       => ("Khong-Duoi",          "No-Extension"),
    "seg.unknown"     => ("Khong-Ro",            "Unknown"),
    "seg.nodate"      => ("Khong-Ro-Ngay",       "Unknown-Date"),
    "seg.nosource"    => ("Khong-Ro-Nguon",      "Unknown-Source"),
    "seg.screenshot"  => ("Anh-Chup-Man-Hinh",   "Screenshots"),
    "seg.noplace"     => ("Khong-Ro-Noi-Chup",   "Unknown-Place"),
    "seg.wrongext"    => ("!Sai-Duoi-File",      "!Wrong-Extension"),
    "seg.from"        => ("Tu-{}",               "From-{}"),
    "seg.downloaded"  => ("Tai-Tu-{}",           "Downloaded-From-{}"),
    "seg.other"       => ("_Khac",               "_Other"),
    "seg.duplicates"  => ("_File-Trung-Lap",     "_Duplicate-Files"),
    "seg.nearDupes"   => ("_Anh-Gan-Giong",      "_Similar-Images"),
    "seg.unnamed"     => ("_Khong-Ten",          "_Unnamed"),

    // ═══════════════════════════════════ MẪU DỰNG SẴN ═══════════════════════

    "pre.downloads.name" => ("Dọn thư mục Tải về", "Clean up Downloads"),
    "pre.downloads.desc" => ("Chia theo loại file, rồi tách file mới với file cũ.",
                             "Group by file type, then separate recent files from old ones."),
    "pre.photos.name"    => ("Xếp ảnh theo ngày chụp", "Sort photos by date taken"),
    "pre.photos.desc"    => ("Ảnh chụp màn hình để riêng, ảnh thật xếp theo năm và tháng chụp.",
                             "Screenshots go to their own folder, real photos by year and month taken."),
    "pre.projects.name"  => ("Gom theo từng công việc", "Group by project"),
    "pre.projects.desc"  => ("Tự nhìn tên file để gom các file cùng một việc, rồi chia theo loại.",
                             "Reads file names to group files belonging together, then splits by type."),
    "pre.company.name"   => ("Xếp giấy tờ công ty", "Sort business paperwork"),
    "pre.company.desc"   => ("Theo năm, theo quý, rồi theo loại giấy tờ: hợp đồng, hoá đơn, báo cáo.",
                             "By year, by quarter, then by document type: contracts, invoices, reports."),
    "pre.diskfull.name"  => ("Tìm cái gì đang chiếm chỗ", "Find what is eating your disk"),
    "pre.diskfull.desc"  => ("Tách riêng file nặng để bạn thấy ngay cái gì làm đầy ổ đĩa.",
                             "Separates the heavy files so you can see what filled up the drive."),
    "pre.media.name"     => ("Xếp phim và nhạc", "Sort video and music"),
    "pre.media.desc"     => ("Theo loại, độ nét, rồi theo năm.",
                             "By type, by resolution, then by year."),
    "pre.archive.name"   => ("Cất file lâu không dùng", "Archive what you never open"),
    "pre.archive.desc"   => ("Tách file lâu rồi bạn chưa mở ra một chỗ riêng, rồi chia theo năm.",
                             "Moves files you have not opened in a long time aside, then splits by year."),
    // "pre.dupes.*" da bo: viec tim file trung khong con la mot kieu SAP XEP,
    // no co man hinh rieng "Tim file & anh trung".

    // ═══════════════════════════ DANH MỤC CÁCH CHIA ═════════════════════════

    "cat.g.common"  => ("Thường dùng nhất",            "Most used"),
    "cat.g.photo"   => ("Dành cho ảnh và video",       "For photos and video"),
    "cat.g.work"    => ("Dành cho giấy tờ công việc",  "For business paperwork"),
    "cat.g.rare"    => ("Ít dùng hơn",                 "Less common"),

    "cat.TYPE.n"    => ("Loại file", "File type"),
    "cat.TYPE.d"    => ("Hình ảnh, Video, Tài liệu, Nhạc... Bạn sửa được bảng này trong Cài đặt.",
                        "Images, Video, Documents, Audio... You can edit this table in Settings."),
    "cat.AUTO_PROJECT.n" => ("Tự gom theo tên giống nhau", "Auto-group by similar names"),
    "cat.AUTO_PROJECT.d" => ("Phần mềm tự nhìn ra các file cùng một việc, hiểu cả tiếng Việt có dấu.",
                             "Works out which files belong together from their names, accents and all."),
    "cat.TIME_MODIFIED.n" => ("Ngày sửa file lần cuối", "Date last modified"),
    "cat.TIME_MODIFIED.d" => ("Chia theo năm, tháng, quý hoặc tuần.",
                              "Split by year, month, quarter or week."),
    "cat.TIME_RELATIVE.n" => ("File mới hay cũ", "How recent the file is"),
    "cat.TIME_RELATIVE.d" => ("Hôm nay, 7 ngày qua, 30 ngày qua, 3 tháng qua, năm nay, cũ hơn 1 năm.",
                              "Today, past 7 days, past 30 days, past 3 months, this year, over a year old."),
    "cat.SIZE_TIER.n" => ("File nặng hay nhẹ", "How big the file is"),
    "cat.SIZE_TIER.d" => ("Chia 5 mức: trên 1GB, 100MB, 10MB, 1MB, và nhỏ hơn.",
                          "Five bands: over 1GB, 100MB, 10MB, 1MB, and smaller."),
    "cat.ALPHABET.n" => ("Chữ cái đầu của tên", "First letter of the name"),
    "cat.ALPHABET.d" => ("A tới Z. Chữ có dấu tự quy về chữ không dấu: Ánh vào A, Đông vào Đ.",
                         "A to Z. Accented letters fold to their base letter."),

    "cat.TIME_TAKEN.n" => ("Ngày chụp thật của ảnh", "Date the photo was actually taken"),
    "cat.TIME_TAKEN.d" => ("Đọc ngày chụp ghi sẵn trong ảnh. Chính xác hơn ngày sửa file vì chép qua lại không làm sai.",
                           "Reads the capture date stored inside the photo. More reliable than the file date, which copying ruins."),
    "cat.SCREENSHOT_DETECT.n" => ("Tách ảnh chụp màn hình", "Separate screenshots"),
    "cat.SCREENSHOT_DETECT.d" => ("Chỉ ảnh chụp màn hình vào thư mục riêng, ảnh thường để nguyên.",
                                  "Only screenshots move to their own folder, real photos stay put."),
    "cat.EXIF_CAMERA.n" => ("Chụp bằng máy nào", "Which camera took it"),
    "cat.EXIF_CAMERA.d" => ("iPhone 15 Pro, Canon EOS R6... đọc từ trong ảnh.",
                            "iPhone 15 Pro, Canon EOS R6... read from inside the photo."),
    "cat.EXIF_GPS_PLACE.n" => ("Nơi chụp ảnh", "Where the photo was taken"),
    "cat.EXIF_GPS_PLACE.d" => ("Đọc toạ độ GPS trong ảnh rồi xếp theo thành phố: Da-Nang, Tokyo... Chỉ ảnh chụp bằng điện thoại có bật định vị mới có.",
                               "Reads the GPS in the photo and sorts by city: Da-Nang, Tokyo... Only works on phone photos with location on."),
    "cat.MEDIA_RESOLUTION.n" => ("Ảnh video nét tới đâu", "How sharp it is"),
    "cat.MEDIA_RESOLUTION.d" => ("8K, 4K, 2K, 1080p, 720p, hoặc thấp hơn.",
                                 "8K, 4K, 2K, 1080p, 720p, or lower."),
    "cat.IMAGE_ORIENTATION.n" => ("Ảnh ngang hay dọc", "Landscape or portrait"),
    "cat.IMAGE_ORIENTATION.d" => ("Ngang, Dọc, hoặc Vuông.", "Landscape, Portrait, or Square."),

    "cat.KEYWORD_RULE.n" => ("Từ khoá trong tên file", "Keyword in the file name"),
    "cat.KEYWORD_RULE.d" => ("Tên file có chữ hợp đồng thì vào thư mục Hợp đồng. Bạn tự soạn bảng từ khoá trong Cài đặt.",
                             "A file named with the word invoice goes to the Invoices folder. You write the keyword table in Settings."),
    "cat.VERSION_GROUP.n" => ("Gom các bản sửa của cùng một file", "Group revisions of the same file"),
    "cat.VERSION_GROUP.d" => ("v1, v2, bản cuối, Bản sao... về chung một chỗ.",
                              "v1, v2, final, Copy... all into one place."),
    "cat.TIME_CREATED.n" => ("Ngày tạo file", "Date the file was created"),
    "cat.TIME_CREATED.d" => ("Theo lúc file được tạo ra lần đầu.", "When the file first came into existence."),
    "cat.ACCESS_HEAT.n" => ("Lâu rồi chưa mở", "How long since you opened it"),
    "cat.ACCESS_HEAT.d" => ("Dưới 30 ngày, dưới 6 tháng, dưới 1 năm, hoặc trên 1 năm.",
                            "Under 30 days, under 6 months, under a year, or over a year."),

    "cat.EXT.n" => ("Đuôi file cụ thể", "Exact file extension"),
    "cat.EXT.d" => ("Mỗi đuôi một thư mục riêng: JPG, PDF, DOCX... không gộp chung.",
                    "One folder per extension: JPG, PDF, DOCX... nothing merged."),
    "cat.SIZE_TIER_AUTO.n" => ("Nặng nhẹ so với nhau", "Big or small relative to each other"),
    "cat.SIZE_TIER_AUTO.d" => ("Tự so trong chính thư mục này để chia nặng nhất, nặng, vừa, nhẹ.",
                               "Compares within this folder to split heaviest, heavy, average, light."),
    "cat.REAL_TYPE.n" => ("Loại thật của file", "What the file really is"),
    "cat.REAL_TYPE.d" => ("Mở ra xem bên trong thật sự là gì, bắt được file bị đổi đuôi sai.",
                          "Looks inside to see what it actually is, catching files with the wrong extension."),
    "cat.DOWNLOAD_SOURCE.n" => ("Tải về từ trang nào", "Which site it was downloaded from"),
    "cat.DOWNLOAD_SOURCE.d" => ("Windows có ghi lại file được tải từ trang web nào.",
                                "Windows records which website each downloaded file came from."),
    "cat.TIME_QUARTER.n" => ("Theo quý", "By quarter"),
    "cat.TIME_QUARTER.d" => ("2026-Q1, 2026-Q2...", "2026-Q1, 2026-Q2..."),
    "cat.TIME_WEEK.n" => ("Theo tuần", "By week"),
    "cat.TIME_WEEK.d" => ("2026-W31, 2026-W32...", "2026-W31, 2026-W32..."),
    "cat.LANGUAGE_SCRIPT.n" => ("Tên file viết bằng tiếng gì", "What language the name is in"),
    "cat.LANGUAGE_SCRIPT.d" => ("Tiếng Việt, Latin, Trung, Nhật, Hàn, Nga.",
                                "Vietnamese, Latin, Chinese, Japanese, Korean, Russian."),
    "cat.ORIGIN_FOLDER.n" => ("Trước đây nằm ở đâu", "Where it used to live"),
    "cat.ORIGIN_FOLDER.d" => ("Giữ lại dấu vết chỗ cũ, ví dụ Tu-Downloads.",
                              "Keeps a trace of the old location, e.g. From-Downloads."),
    "cat.LITERAL.n" => ("Một thư mục do bạn đặt tên", "A folder you name yourself"),
    "cat.LITERAL.d" => ("Gõ tên thư mục, mọi file sẽ vào đó.",
                        "Type a folder name and everything goes in there."),

    // ═══════════════════════════════ THÔNG BÁO ══════════════════════════════

    "msg.rootAtLarge"   => ("{}% số file nằm lộn xộn ngay ở ngoài cùng, chưa được xếp vào thư mục nào.",
                            "{}% of files sit loose at the top level, not sorted into any folder."),
    "msg.rootSome"      => ("{}% số file còn nằm lộn xộn ở ngoài cùng.",
                            "{}% of files are still loose at the top level."),
    "msg.unnamed"       => ("{}% số file mang tên vô nghĩa kiểu IMG_1234 hay Untitled, sau này rất khó tìm lại.",
                            "{}% of files have meaningless names like IMG_1234 or Untitled, which makes them hard to find later."),
    "msg.cold"          => ("{}% số file cả năm nay bạn chưa mở lần nào, nên gom vào một chỗ lưu trữ riêng.",
                            "{}% of files have not been opened in a year, so they belong in an archive."),
    "msg.deep"          => ("Thư mục lồng nhau tới {} lớp, sâu quá thì tìm bằng mắt rất mệt.",
                            "Folders nest {} levels deep, which is too deep to find anything by eye."),
    "msg.flat"          => ("Gần như không có thư mục con nào, mọi thứ đổ dồn vào một chỗ.",
                            "There are almost no subfolders, everything is piled into one place."),
    "msg.driveFull"     => ("Ổ {} đã dùng hết {}% dung lượng, chỉ còn trống {}.",
                            "Drive {} is {}% full, with only {} left."),
    "msg.appsProtected" => ("{} thư mục phần mềm được để nguyên tại chỗ, chuyển đi là phần mềm hỏng.",
                            "{} application folders were left alone, because moving them breaks the software."),
    "msg.tidy"          => ("Không có vấn đề gì đáng ngại.", "Nothing here looks like a problem."),

    "msg.sysDrive"      => ("Đây là ổ cài Windows. Sắp xếp cả ổ sẽ làm hỏng máy. Bạn hãy chọn một thư mục con cụ thể bên trong.",
                            "This is the Windows drive. Organising the whole drive would break your computer. Pick a specific folder inside it instead."),
    "msg.sysDriveShort" => ("Đây là ổ cài Windows. Bạn chọn một thư mục con bên trong thay vì cả ổ.",
                            "This is the Windows drive. Pick a folder inside it rather than the whole drive."),
    "msg.wholeDrive"    => ("Bạn đang chọn cả ổ {}. Thư mục hệ thống và phần mềm đã cài sẽ tự động được bỏ qua. Dù vậy vẫn nên xem kỹ bước kiểm tra.",
                            "You are selecting the whole {} drive. System folders and installed software are skipped automatically. Even so, look carefully at the check step."),
    "msg.cdrom"         => ("Ổ đĩa CD/DVD chỉ đọc được, không sắp xếp được.",
                            "A CD/DVD drive is read-only, nothing can be organised on it."),
    "msg.cdromShort"    => ("Ổ đĩa CD/DVD chỉ đọc được, không sửa được.",
                            "A CD/DVD drive is read-only."),
    "msg.netDrive"      => ("Đây là ổ trên mạng. Làm sẽ chậm và phụ thuộc đường truyền.",
                            "This is a network drive. Work will be slow and depends on the connection."),
    "msg.notFolder"     => ("Chỗ này không phải thư mục.", "This is not a folder."),
    "msg.cantRead"      => ("Không đọc được thư mục ({})", "Could not read the folder ({})"),
    "msg.protectedDir"  => ("Thư mục hệ thống được bảo vệ: {}", "This is a protected system folder: {}"),
    "msg.importantDir"  => ("Đây là thư mục quan trọng ({}). Hãy xem kỹ bước kiểm tra trước khi làm.",
                            "This is an important folder ({}). Look carefully at the check step before running."),
    "msg.recursion"     => ("Thư mục bạn chọn lại nằm bên trong chỗ đích. Làm vậy sẽ lặp vô tận.",
                            "The folder you picked sits inside the destination. That would loop forever."),
    "msg.destInside"    => ("Chỗ đích nằm bên trong thư mục bạn chọn. Phần mềm sẽ tự bỏ nó ra khỏi phạm vi.",
                            "The destination sits inside the folder you picked. It will be left out of the scan automatically."),

    "msg.nothingPicked" => ("Bạn chưa chọn file nào.", "You have not selected any files."),
    "msg.noWrite"       => ("Máy không cho ghi vào chỗ này: {}", "The system will not let anything be written here: {}"),
    "msg.longPath"      => ("{} file có đường dẫn quá dài nên sẽ được để yên.",
                            "{} files have paths that are too long, so they will be left alone."),
    "msg.locked"        => ("{} file đang mở trong phần mềm khác như Word, Excel hay Photoshop. Những file này sẽ được để yên, các file còn lại vẫn xếp bình thường.",
                            "{} files are open in another program such as Word, Excel or Photoshop. Those will be left alone, everything else still gets sorted."),
    "msg.noSpace"       => ("Ổ {} không đủ chỗ trống. Cần {} mà chỉ còn {}.",
                            "Drive {} does not have enough free space. It needs {} but only {} is left."),
    "msg.destClash"     => ("Lỗi bên trong phần mềm: {} file bị trùng chỗ đích.",
                            "Internal error: {} files were assigned the same destination."),
    "msg.preflightOk"   => ("Kiểm tra an toàn đã qua.", "Safety check passed."),

    "msg.skipApp"       => ("Thư mục phần mềm ({}). Được để nguyên tại chỗ.",
                            "Application folder ({}). Left exactly where it is."),
    "msg.skipSystem"    => ("Thứ quan trọng của ổ đĩa. Phần mềm không bao giờ đụng vào.",
                            "Something the drive needs. This is never touched."),
    "msg.skipLink"      => ("Đây là lối tắt trỏ sang chỗ khác, không đi theo.",
                            "This is a shortcut pointing somewhere else, so it is not followed."),
    "msg.skipCloud"     => ("File này còn trên mây, chưa tải về máy. Bỏ qua để khỏi tốn công tải.",
                            "This file is still in the cloud, not on the computer. Skipped so nothing gets downloaded."),
    "msg.skipDeep"      => ("Thư mục lồng nhau quá 64 lớp nên dừng lại.",
                            "Folders nest more than 64 levels deep, so scanning stopped here."),
    "msg.skipNoInfo"    => ("Không đọc được thông tin ({})", "Could not read its details ({})"),
    "msg.skipNoDir"     => ("Không đọc được thư mục ({})", "Could not read the folder ({})"),

    "msg.copyMismatchSize" => ("Bản chép sang chỗ mới bị thiếu. Đã huỷ bỏ, file gốc vẫn còn nguyên.",
                               "The copy at the new location came out incomplete. It was cancelled and the original is untouched."),
    "msg.copyMismatch"     => ("Bản chép sang chỗ mới bị sai nội dung. Đã huỷ bỏ, file gốc vẫn còn nguyên.",
                               "The copy at the new location did not match. It was cancelled and the original is untouched."),
    "msg.inRecycleBin"     => ("File này đã vào Thùng rác. Bạn mở Thùng rác của Windows để lấy lại.",
                               "This file went to the Recycle Bin. Open the Windows Recycle Bin to get it back."),
    "msg.movedAway"        => ("Không tìm thấy file ở chỗ mới. Có thể bạn đã tự chuyển hoặc xoá nó.",
                               "The file is no longer at its new location. You may have moved or deleted it yourself."),
    "msg.oldSpotTaken"     => ("Chỗ cũ giờ có file khác rồi, nên không ghi đè lên.",
                               "Another file now occupies the old spot, so nothing was overwritten."),
    "msg.hardlinkSameDrive"=> ("Lối tắt chỉ tạo được trong cùng một ổ đĩa.",
                               "A shortcut like this can only be made within the same drive."),
    "msg.badDest"          => ("Chỗ cần chuyển tới không hợp lệ.", "The destination path is not valid."),
    "msg.recycleFailed"    => ("Không bỏ vào Thùng rác được: {}", "Could not move it to the Recycle Bin: {}"),
    "msg.dupReason"        => ("Trùng lặp nội dung", "Identical content"),
    "msg.nearReason"       => ("Ảnh gần giống, cần bạn xem lại", "Similar image, needs your review"),
    "msg.renameReason"     => ("Đổi tên theo mẫu", "Renamed by your pattern"),
    "prog.nearDupes"       => ("Đang so ảnh xem có tấm nào gần giống nhau", "Comparing photos for near matches"),
    "msg.alreadyThere"     => ("Đã nằm đúng chỗ", "Already in the right place"),
    "msg.noLayers"         => ("Không phân tầng", "No grouping"),
    "msg.withSidecar"      => ("{} (đi kèm file chính)", "{} (kept with its main file)"),

    "msg.tooManyFolders"   => ("Cách này tạo ra tới {} thư mục, nhiều hơn mức {} bạn đặt. Hãy kéo thanh gom nhóm về phía ít thư mục, hoặc bớt một mức chia.",
                               "This would create {} folders, more than the {} you allow. Drag the grouping slider toward fewer folders, or remove one level."),
    "msg.tooMuchOther"     => ("Có {} trong {} file không gom được vào nhóm nào nên sẽ vào thư mục \"{}\". Hãy kéo thanh gom nhóm về phía nhiều thư mục nhỏ.",
                               "{} of {} files could not be grouped and will land in \"{}\". Drag the grouping slider toward more, smaller folders."),

    "msg.noPlan"           => ("Chưa có kế hoạch nào.", "There is no plan yet."),
    "msg.noScan"           => ("Chưa quét thư mục nào.", "Nothing has been scanned yet."),
    "msg.noFolderPicked"   => ("Chưa chọn thư mục nào.", "No folder has been picked."),
    "msg.noOps"            => ("Không có thao tác nào để thực hiện.", "There is nothing to do."),
    "msg.noSession"        => ("Không tìm thấy nhật ký của phiên này.", "The record for this run could not be found."),
    "msg.noSessionFound"   => ("Không tìm thấy phiên này.", "This run could not be found."),
    "msg.journalFailed"    => ("Không tạo được nhật ký: {}", "Could not create the record file: {}"),
    "msg.journalWriteFail" => ("Không ghi được nhật ký: {}", "Could not write the record file: {}"),
    "msg.noReport"         => ("Chưa có kế hoạch nào để xuất.", "There is no plan to export yet."),

    "msg.sweeping"      => ("Đang dọn các thư mục trống còn lại", "Clearing out the empty folders left behind"),
    "msg.skipByCheck"   => ("Bị bỏ qua bởi bước kiểm tra an toàn", "Skipped by the safety check"),
    "msg.srcGone"       => ("File nguồn không còn tồn tại", "The source file no longer exists"),
    "msg.journalRetry"  => ("Không tạo được file nhật ký mới sau 64 lần thử",
                            "Could not create a new record file after 64 attempts"),

    // Nhãn biểu đồ
    "lbl.atRoot"        => ("(ngay ở ngoài cùng)", "(loose at the top)"),
    "lbl.misc"          => ("(khác)", "(other)"),
    "lbl.otherBucket"   => ("Khác", "Other"),

    // Dòng trạng thái lúc đang chạy
    "prog.scanning"     => ("Đang xem có những gì trong thư mục", "Looking through the folder"),
    "prog.scanningAt"   => ("Đang xem: {}", "Looking at: {}"),
    "prog.analyzing"    => ("Đang xem xét tình trạng", "Working out the state of things"),
    "prog.scanDone"     => ("Xem xong", "Finished looking"),
    "prog.planDone"     => ("Đã tính xong cách sắp xếp", "Worked out how to sort it"),
    "prog.readMedia"    => ("Đang đọc thông tin trong ảnh và video", "Reading the details inside photos and video"),
    "prog.clustering"   => ("Đang tìm xem file nào thuộc cùng một việc", "Working out which files belong together"),
    "prog.findDupes"    => ("Đang tìm file bị lưu trùng", "Looking for files stored more than once"),
    "prog.hashing"      => ("Đang đối chiếu nội dung từng file", "Comparing what is inside each file"),
    "prog.applying"     => ("Đang xử lý: {}", "Working on: {}"),
    "prog.undoing"      => ("Đang trả file về chỗ cũ", "Putting the files back"),
    "prog.done"         => ("Xong", "Done"),
    "prog.undone"       => ("Đã trả về như cũ", "Everything is back as it was"),

    // Nhãn dùng trong hộp thoại hệ thống và báo cáo
    "ui.pickFolder"     => ("Chọn thư mục cần sắp xếp", "Pick a folder to organise"),
    "ui.pickDest"       => ("Chọn thư mục đích", "Pick a destination folder"),
    "ui.reportTitle"    => ("Báo cáo sắp xếp thư mục", "Folder organisation report"),
    "ui.reportMade"     => ("Tạo lúc", "Created"),
    "ui.reportMode"     => ("Chế độ", "Mode"),
    "ui.colAction"      => ("Thao tác", "Action"),
    "ui.colFrom"        => ("Nguồn", "From"),
    "ui.colTo"          => ("Đích", "To"),
    "ui.colSize"        => ("Kích thước", "Size"),
    "ui.colReason"      => ("Lý do", "Reason"),
    "ui.sumTotal"       => ("Tổng thao tác", "Total actions"),
    "ui.sumMoves"       => ("Di chuyển", "Moved"),
    "ui.sumRenames"     => ("Đổi tên", "Renamed"),
    "ui.sumDupes"       => ("Trùng lặp", "Duplicates"),
    "ui.sumFolders"     => ("Thư mục mới", "New folders"),
    "ui.sumWasted"      => ("Dung lượng lãng phí", "Wasted space"),
    "ui.driveLocal"     => ("Ổ đĩa cục bộ", "Local Disk"),
    "ui.driveRemovable" => ("Ổ rời", "Removable Drive"),
    "ui.driveNetwork"   => ("Ổ mạng", "Network Drive"),

    // Khoá không có trong bảng là lỗi lập trình, không phải tình huống người dùng.
    // Ở bản gỡ lỗi thì dừng ngay để test bắt được; ở bản phát hành thì hiện dấu hỏi
    // chứ không làm sập ứng dụng.
    _ => {
        debug_assert!(false, "thiếu chuỗi i18n cho khoá: {}", key);
        ("(?)", "(?)")
    }
    }
}

/// Ngôn ngữ là trạng thái toàn cục nên các test đụng tới nó phải chạy nối tiếp,
/// không thì test này đổi ngôn ngữ giữa lúc test kia đang kiểm.
///
/// Không gắn `#[cfg(test)]` vì test tích hợp liên kết với bản lib biên dịch thường,
/// ở đó cờ test không bật nên sẽ không thấy khoá này.
#[doc(hidden)]
pub static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doi_ngon_ngu_doi_luon_ten_thu_muc() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_lang(Lang::Vi);
        assert_eq!(t("grp.images"), "01-Hinh-Anh");
        assert_eq!(month(2), "03-Thang-Ba");
        set_lang(Lang::En);
        assert_eq!(t("grp.images"), "01-Images");
        assert_eq!(month(2), "03-March");
        set_lang(Lang::Vi);
    }

    #[test]
    fn thay_tham_so_theo_thu_tu() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_lang(Lang::En);
        assert_eq!(tf("seg.downloaded", &["example.com"]), "Downloaded-From-example.com");
        assert_eq!(tf("msg.noSpace", &["D", "2 GB", "1 GB"]),
                   "Drive D does not have enough free space. It needs 2 GB but only 1 GB is left.");
        set_lang(Lang::Vi);
    }

    /// Tên thư mục tạo ra trên ổ đĩa phải an toàn trên mọi hệ thống tập tin
    #[test]
    fn moi_ten_thu_muc_deu_la_ascii_hop_le() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEYS: &[&str] = &[
            "grp.images", "grp.raw", "grp.video", "grp.audio", "grp.docs", "grp.sheets",
            "grp.slides", "grp.archives", "grp.installers", "grp.design", "grp.code",
            "grp.cad", "grp.fonts", "grp.other",
            "kw.contracts", "kw.invoices", "kw.reports", "kw.hr", "kw.plans", "kw.quotes",
            "kw.legal", "kw.finance",
            "seg.size.huge", "seg.size.large", "seg.size.medium", "seg.size.small", "seg.size.tiny",
            "seg.rel.today", "seg.rel.week", "seg.rel.month", "seg.rel.quarter", "seg.rel.year",
            "seg.rel.older", "seg.heat.hot", "seg.heat.warm", "seg.heat.cool", "seg.heat.frozen",
            "seg.res.8k", "seg.res.4k", "seg.res.2k", "seg.res.1080", "seg.res.720", "seg.res.low",
            "seg.orient.land", "seg.orient.port", "seg.orient.sq",
            "seg.script.ja", "seg.script.ko", "seg.script.zh", "seg.script.ru", "seg.script.vi",
            "seg.script.latin", "seg.project", "seg.folder", "seg.noext", "seg.unknown",
            "seg.nodate", "seg.nosource", "seg.screenshot", "seg.other", "seg.duplicates",
            "seg.unnamed",
        ];
        for l in [Lang::Vi, Lang::En] {
            set_lang(l);
            for k in KEYS {
                let s = t(k);
                assert!(s.is_ascii(), "{} ({:?}) không phải ASCII: {}", k, l, s);
                assert!(!s.is_empty(), "{} rỗng", k);
                for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
                    assert!(!s.contains(c), "{} chứa ký tự Windows cấm '{}': {}", k, c, s);
                }
                assert!(!s.ends_with('.') && !s.ends_with(' '), "{} kết thúc sai: {}", k, s);
            }
            for m in 0..12 {
                assert!(month(m).is_ascii(), "tháng {} không phải ASCII", m);
            }
        }
        set_lang(Lang::Vi);
    }
}
