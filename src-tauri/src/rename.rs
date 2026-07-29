//! Đổi tên hàng loạt theo mẫu do người dùng lắp.
//!
//! NGUY HIỂM NHẤT của cả phần mềm: một mẫu sai áp lên nghìn file là hỏng nghìn tên.
//! Vì vậy module này chỉ TÍNH ra tên mới, tuyệt đối không ghi ổ đĩa. Việc thực thi
//! và hoàn tác đi qua đúng bộ máy nhật ký + undo đã có, nên bấm một nút là về tên cũ.
//!
//! Ba lớp an toàn nằm ngay trong hàm tạo tên:
//!   1. ĐUÔI FILE luôn được giữ nguyên, người dùng không đổi được. Đổi đuôi .jpg
//!      thành thứ khác là làm hỏng liên kết mở file, nên không cho phép.
//!   2. Tên rỗng sau khi xử lý -> lùi về tên gốc, không bao giờ ra tên trống.
//!   3. Ký tự cấm của Windows, tên thiết bị cấm, đuôi dấu chấm/khoảng trắng đều
//!      bị làm sạch qua `sanitize_segment`.
//!
//! Còn chống đè (hai file ra cùng tên) và chống mất file thì do lớp planner +
//! executor lo, y như mọi thao tác khác.

use crate::util::{kebab, sanitize_segment, split_name, strftime, strip_diacritics};
use serde::{Deserialize, Serialize};

/// Một mảnh ghép trong mẫu tên. Cấu trúc phẳng để dựng từ JS cho dễ.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// "text" | "name" | "date" | "taken" | "created" | "folder" | "counter"
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub width: u8,
    #[serde(default)]
    pub start: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transforms {
    pub strip_diacritics: bool,
    pub lowercase: bool,
    /// Khoảng trắng -> gạch nối
    pub kebab: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameSpec {
    pub enabled: bool,
    /// true = đổi tên nhưng để file yên chỗ cũ (mặc định, an toàn nhất).
    /// false = vừa đổi tên vừa sắp xếp theo các tầng.
    #[serde(default = "yes")]
    pub in_place: bool,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub transforms: Transforms,
}

fn yes() -> bool {
    true
}

impl Default for RenameSpec {
    fn default() -> Self {
        RenameSpec {
            enabled: false,
            in_place: true,
            parts: vec![],
            transforms: Transforms::default(),
        }
    }
}

/// Dữ liệu một file cần để dựng tên mới
pub struct RenameCtx<'a> {
    /// Tên gốc đầy đủ, kể cả đuôi
    pub original: &'a str,
    pub mtime: i64,
    pub ctime: i64,
    /// Ngày chụp thật nếu có, không thì 0
    pub taken: i64,
    /// Tên thư mục file sẽ nằm vào
    pub folder: &'a str,
    /// Số thứ tự đã gán sẵn cho file này (planner lo việc đánh số theo thư mục)
    pub counter: u32,
}

/// Tạo tên mới. KHÔNG bao giờ trả về chuỗi rỗng, KHÔNG bao giờ đổi đuôi file.
pub fn render_name(parts: &[Part], transforms: &Transforms, ctx: &RenameCtx) -> String {
    let (orig_stem, ext) = split_name(ctx.original);

    let mut out = String::new();
    for p in parts {
        match p.kind.as_str() {
            "text" => out.push_str(&p.text),
            "name" => out.push_str(&orig_stem),
            "folder" => out.push_str(ctx.folder),
            "date" => out.push_str(&fmt_date(ctx.mtime, &p.format)),
            "created" => out.push_str(&fmt_date(ctx.ctime, &p.format)),
            "taken" => {
                // Ảnh không có ngày chụp thì lùi về ngày sửa, để đừng ra "Khong-Ro-Ngay"
                let t = if ctx.taken > 0 { ctx.taken } else { ctx.mtime };
                out.push_str(&fmt_date(t, &p.format));
            }
            "counter" => {
                let n = ctx.counter.saturating_add(p.start.max(0));
                let w = p.width.clamp(1, 9) as usize;
                out.push_str(&format!("{:0width$}", n, width = w));
            }
            _ => {}
        }
    }

    // --- Biến đổi toàn bộ tên (chưa có đuôi)
    if transforms.strip_diacritics {
        out = strip_diacritics(&out);
    }
    if transforms.lowercase {
        out = out.to_lowercase();
    }

    // Gỡ một mảnh giữa chừng thường để lại dấu ngăn cách lơ lửng ở đầu/cuối
    // ("2026_da-lat_" hoặc "_2026-da-lat"). Cắt TRƯỚC khi làm sạch, để không đụng
    // vào dấu "_" mà làm sạch thêm vào để bảo vệ tên thiết bị cấm (CON -> _CON).
    let trimmed = out.trim_matches(|c| c == '_' || c == '-' || c == ' ' || c == '.');

    // Rỗng sau khi ghép -> lùi về tên gốc, không bao giờ ra tên trống
    let base = if trimmed.is_empty() { orig_stem.as_str() } else { trimmed };

    let mut stem = if transforms.kebab {
        kebab(base)
    } else {
        sanitize_segment(base)
    };
    if stem.is_empty() {
        stem = "file".to_string();
    }

    // --- Đuôi file GIỮ NGUYÊN, không cho người dùng đụng vào
    format!("{}{}", stem, ext)
}

/// Định dạng ngày. Mẫu rỗng thì dùng %Y-%m-%d cho gọn.
fn fmt_date(ms: i64, format: &str) -> String {
    let f = if format.is_empty() { "%Y-%m-%d" } else { format };
    // Trong tên file thì dấu / không tạo thư mục con được, đổi thành gạch nối
    strftime(ms, f).replace(['/', '\\'], "-")
}

/// Sắp xếp các file trong cùng một thư mục để đánh số ổn định.
/// Cùng một tập file thì luôn cho ra cùng thứ tự -> preview khớp lúc chạy thật.
pub fn counter_order_key(name: &str) -> (String, String) {
    let n = crate::util::norm_key(name);
    (n, name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(kind: &str) -> Part {
        Part {
            kind: kind.into(),
            text: String::new(),
            format: String::new(),
            width: 0,
            start: 0,
        }
    }
    fn text(s: &str) -> Part {
        let mut p = part("text");
        p.text = s.into();
        p
    }

    fn ctx<'a>(original: &'a str, folder: &'a str, counter: u32) -> RenameCtx<'a> {
        RenameCtx {
            original,
            // 2026-03-15 12:00 local, cố định để test khỏi phụ thuộc máy
            mtime: 1_773_000_000_000,
            ctime: 1_773_000_000_000,
            taken: 0,
            folder,
            counter,
        }
    }

    #[test]
    fn ghep_ngay_ten_thu_muc_va_so_thu_tu() {
        let parts = vec![
            {
                let mut p = part("date");
                p.format = "%Y-%m-%d".into();
                p
            },
            text("_"),
            part("folder"),
            text("_"),
            {
                let mut p = part("counter");
                p.width = 3;
                p.start = 1;
                p
            },
        ];
        let out = render_name(&parts, &Transforms::default(), &ctx("IMG_4821.jpg", "Da-Lat", 0));
        assert!(out.ends_with(".jpg"), "phải giữ đuôi .jpg: {}", out);
        assert!(out.contains("Da-Lat"), "{}", out);
        assert!(out.contains("001"), "số thứ tự phải là 001: {}", out);
        assert!(out.starts_with("2026-03"), "{}", out);
    }

    #[test]
    fn duoi_file_khong_bao_gio_bi_doi() {
        // Người dùng cố nhét ".exe" vào text cũng không làm đổi được đuôi thật
        let parts = vec![text("virus.exe")];
        let out = render_name(&parts, &Transforms::default(), &ctx("anh.jpg", "F", 0));
        assert_eq!(out, "virus.exe.jpg", "đuôi .jpg gốc phải được giữ, .exe thành phần tên: {}", out);
    }

    #[test]
    fn ten_rong_thi_lui_ve_ten_goc() {
        let parts = vec![text("   ")];
        let out = render_name(&parts, &Transforms::default(), &ctx("bao cao.pdf", "F", 0));
        assert_eq!(out, "bao cao.pdf", "tên rỗng phải lùi về tên gốc: {}", out);
    }

    #[test]
    fn khong_co_manh_nao_thi_giu_nguyen_ten_goc() {
        let out = render_name(&[], &Transforms::default(), &ctx("Tài Liệu.docx", "F", 0));
        assert_eq!(out, "Tài Liệu.docx");
    }

    #[test]
    fn bo_dau_chu_thuong_gach_noi() {
        let parts = vec![part("name")];
        let tr = Transforms {
            strip_diacritics: true,
            lowercase: true,
            kebab: true,
        };
        let out = render_name(&parts, &tr, &ctx("Hợp Đồng Thuê Nhà.PDF", "F", 0));
        assert_eq!(out, "hop-dong-thue-nha.PDF", "{}", out);
    }

    #[test]
    fn ky_tu_cam_windows_bi_lam_sach() {
        let parts = vec![text("a/b:c*d")];
        let out = render_name(&parts, &Transforms::default(), &ctx("x.txt", "F", 0));
        for c in ['/', ':', '*', '<', '>', '"', '|', '?', '\\'] {
            assert!(!out.contains(c), "còn ký tự cấm '{}': {}", c, out);
        }
    }

    #[test]
    fn cat_dau_ngan_cach_lo_lung_o_dau_cuoi() {
        // Gỡ mảnh số thứ tự để lại "date_folder_" -> phải thành "date_folder"
        let parts = vec![
            {
                let mut p = part("date");
                p.format = "%Y".into();
                p
            },
            text("_"),
            part("folder"),
            text("_"),
        ];
        let out = render_name(&parts, &Transforms::default(), &ctx("x.jpg", "Da-Lat", 0));
        assert_eq!(out, "2026_Da-Lat.jpg", "dấu _ lơ lửng cuối phải bị cắt: {}", out);
    }

    #[test]
    fn so_thu_tu_dung_do_rong() {
        let mut p = part("counter");
        p.width = 4;
        p.start = 0;
        let out = render_name(&[p], &Transforms::default(), &ctx("x.jpg", "F", 41));
        assert_eq!(out, "0041.jpg");
    }
}
