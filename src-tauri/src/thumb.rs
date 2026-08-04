//! Anh thu nho, de nguoi dung TU NHIN thay may tam anh bi coi la gan giong nhau.
//!
//! Truoc day tinh nang anh gan giong bat nguoi dung tin vao mot con so ho khong
//! thay: phan mem gom anh vao mot thu muc rieng roi bao "tu xem lai di". Mo thu
//! muc do bang Explorer thi cung khong biet tam nao bi coi la trung voi tam nao.
//!
//! RIENG TU la rang buoc dau tien o day, khong phai hieu nang:
//!   1. Anh duoc doc va thu nho NGAY TRONG LOI Rust roi dua sang giao dien duoi
//!      dang du lieu nhung. Lop giao dien khong bao gio cam duong dan de tu doc
//!      dia, nen khong phai bat giao thuc `asset:` cua Tauri — thu von mo quyen
//!      doc file cho trinh duyet nhung.
//!   2. KHONG ghi anh nho ra dia. Tat phan mem la het, khong de lai cache anh
//!      rieng tu trong %APPDATA%.
//!   3. KHONG dua anh vao bao cao xuat ra, vi bao cao la file nguoi dung co the
//!      gui cho nguoi khac.

use std::path::Path;

/// Canh dai nhat cua anh thu nho. Du nhin ra hai tam co phai cung mot anh khong,
/// ma van du nho de vai tram tam khong phinh bo nho.
pub const THUMB_MAX: u32 = 160;

/// Base64 chuan (RFC 4648). Tu viet cho khoi keo them mot thu vien chi de lam
/// dung mot viec nay.
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((*c.get(1).unwrap_or(&0) as u32) << 8)
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Anh thu nho dang `data:` URI. `None` khi file khong doc hoac khong giai ma duoc
/// — luc do giao dien hien o trong thay vi bao loi, vi day chi la phan minh hoa.
pub fn data_uri(path: &Path) -> Option<String> {
    let img = image::open(path).ok()?;
    let small = img.thumbnail(THUMB_MAX, THUMB_MAX); // giu nguyen ty le khung
    let rgb = small.to_rgb8(); // JPEG khong nhan kenh trong suot
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 70)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(format!("data:image/jpeg;base64,{}", base64(&buf)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_dung_chuan() {
        // Cac vi du trong RFC 4648, gom ca ba truong hop du/thieu byte cuoi
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn anh_that_ra_data_uri_jpeg() {
        let dir = std::env::temp_dir().join(format!("foldu-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("to.png");

        // Anh 400x300 co hoa tiet, thu nho phai ra dung ty le 160x120
        let mut img = image::RgbImage::new(400, 300);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        img.save(&p).unwrap();

        let uri = data_uri(&p).expect("phai tao duoc anh thu nho");
        assert!(uri.starts_with("data:image/jpeg;base64,"));

        // Giai ma nguoc lai de chac chan day la JPEG hop le, dung kich thuoc
        let b64 = uri.trim_start_matches("data:image/jpeg;base64,");
        let raw = decode_b64(b64);
        let back = image::load_from_memory(&raw).expect("phai la JPEG hop le");
        assert_eq!((back.width(), back.height()), (160, 120));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_hong_thi_tra_none_chu_khong_no() {
        let dir = std::env::temp_dir().join(format!("foldu-thumb-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("khong-phai-anh.jpg");
        std::fs::write(&p, b"day khong phai anh").unwrap();

        assert!(data_uri(&p).is_none());
        assert!(data_uri(&dir.join("khong-ton-tai.jpg")).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Giai ma base64 — chi dung trong test de kiem chung chieu ma hoa
    fn decode_b64(s: &str) -> Vec<u8> {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let idx = |c: u8| T.iter().position(|&t| t == c).unwrap() as u32;
        let b: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
        let mut out = Vec::new();
        for c in b.chunks(4) {
            let mut n = 0u32;
            for (i, &ch) in c.iter().enumerate() {
                n |= idx(ch) << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if c.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if c.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }
}
