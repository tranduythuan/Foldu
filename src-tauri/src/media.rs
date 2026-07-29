//! Doc metadata media: EXIF (ngay chup that, may anh), kich thuoc anh,
//! nhan dang loai file that qua magic bytes, nguon tai ve tu ADS Zone.Identifier.
//!
//! Chi doc phan dau file - khong nap ca file vao bo nho.

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Duoi file that suy ra tu magic bytes (bat file bi doi duoi sai)
    pub real_ext: Option<String>,
    pub real_kind: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Ngay chup that (EXIF DateTimeOriginal), milli giay
    pub taken_ms: Option<i64>,
    pub camera: Option<String>,
    /// Ten noi chup, tra tu toa do GPS trong anh (vd "Da-Nang"). None neu khong co GPS
    /// hoac khong co thanh pho nao du gan.
    pub place: Option<String>,
}

// ---------------------------------------------------------------- Magic bytes

struct Sig {
    ext: &'static str,
    kind: &'static str,
    bytes: &'static [u8],
}

const SIGS: &[Sig] = &[
    Sig { ext: "jpg",  kind: "image",      bytes: &[0xFF, 0xD8, 0xFF] },
    Sig { ext: "png",  kind: "image",      bytes: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] },
    Sig { ext: "gif",  kind: "image",      bytes: b"GIF8" },
    Sig { ext: "bmp",  kind: "image",      bytes: b"BM" },
    Sig { ext: "psd",  kind: "design",     bytes: b"8BPS" },
    Sig { ext: "pdf",  kind: "document",   bytes: b"%PDF" },
    Sig { ext: "rtf",  kind: "document",   bytes: b"{\\rt" },
    Sig { ext: "doc",  kind: "document",   bytes: &[0xD0, 0xCF, 0x11, 0xE0] },
    Sig { ext: "zip",  kind: "archive",    bytes: &[0x50, 0x4B, 0x03, 0x04] },
    Sig { ext: "rar",  kind: "archive",    bytes: b"Rar!" },
    Sig { ext: "7z",   kind: "archive",    bytes: &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] },
    Sig { ext: "gz",   kind: "archive",    bytes: &[0x1F, 0x8B] },
    Sig { ext: "exe",  kind: "executable", bytes: b"MZ" },
    Sig { ext: "mp3",  kind: "audio",      bytes: b"ID3" },
    Sig { ext: "flac", kind: "audio",      bytes: b"fLaC" },
    Sig { ext: "ogg",  kind: "audio",      bytes: b"OggS" },
];

/// Nhan dang loai file that tu 16 byte dau
pub fn sniff(head: &[u8]) -> Option<(&'static str, &'static str)> {
    for s in SIGS {
        if head.len() >= s.bytes.len() && &head[..s.bytes.len()] == s.bytes {
            return Some((s.ext, s.kind));
        }
    }
    if head.len() >= 12 {
        if &head[4..8] == b"ftyp" {
            let brand = &head[8..12];
            if brand.starts_with(b"hei") || brand.starts_with(b"mif") {
                return Some(("heic", "image"));
            }
            return Some(("mp4", "video"));
        }
        if &head[0..4] == b"RIFF" {
            return match &head[8..12] {
                b"WEBP" => Some(("webp", "image")),
                b"WAVE" => Some(("wav", "audio")),
                b"AVI " => Some(("avi", "video")),
                _ => None,
            };
        }
        if head[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
            return Some(("mkv", "video"));
        }
    }
    None
}

// ---------------------------------------------------------------------- EXIF

/// Đổi một toạ độ GPS trong EXIF (độ, phút, giây + hướng) thành số thập phân.
fn read_gps_coord(
    exif: &exif::Exif,
    coord_tag: exif::Tag,
    ref_tag: exif::Tag,
    neg_ref: &str,
) -> Option<f64> {
    let f = exif.get_field(coord_tag, exif::In::PRIMARY)?;
    let dms = match &f.value {
        exif::Value::Rational(v) if v.len() >= 3 => v,
        _ => return None,
    };
    let deg = dms[0].to_f64();
    let min = dms[1].to_f64();
    let sec = dms[2].to_f64();
    let mut val = deg + min / 60.0 + sec / 3600.0;

    // Hướng N/S/E/W nằm ở trường ref riêng; S và W là âm
    if let Some(rf) = exif.get_field(ref_tag, exif::In::PRIMARY) {
        let r = rf.display_value().to_string();
        if r.trim().eq_ignore_ascii_case(neg_ref) {
            val = -val;
        }
    }
    if val.is_finite() {
        Some(val)
    } else {
        None
    }
}

fn read_gps(exif: &exif::Exif) -> Option<(f64, f64)> {
    let lat = read_gps_coord(exif, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef, "S")?;
    let lon = read_gps_coord(exif, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef, "W")?;
    // Toạ độ (0,0) giữa vịnh Guinea gần như luôn là lỗi/thiếu dữ liệu, bỏ qua
    if lat.abs() < 0.001 && lon.abs() < 0.001 {
        return None;
    }
    Some((lat, lon))
}

fn read_exif(path: &Path) -> (Option<i64>, Option<String>, Option<(f64, f64)>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None, None),
    };
    let mut reader = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return (None, None, None),
    };

    // Ngay chup that: uu tien DateTimeOriginal, sau do DateTimeDigitized
    let mut taken = None;
    for tag in [exif::Tag::DateTimeOriginal, exif::Tag::DateTimeDigitized] {
        if taken.is_some() {
            break;
        }
        if let Some(f) = exif.get_field(tag, exif::In::PRIMARY) {
            if let exif::Value::Ascii(ref vs) = f.value {
                if let Some(v) = vs.first() {
                    if let Ok(dt) = exif::DateTime::from_ascii(v) {
                        if dt.year > 1980 {
                            if let Some(t) = Local
                                .with_ymd_and_hms(
                                    dt.year as i32,
                                    dt.month as u32,
                                    dt.day as u32,
                                    dt.hour as u32,
                                    dt.minute as u32,
                                    dt.second as u32,
                                )
                                .single()
                            {
                                taken = Some(t.timestamp_millis());
                            }
                        }
                    }
                }
            }
        }
    }

    let get = |tag| {
        exif.get_field(tag, exif::In::PRIMARY).map(|f| {
            f.display_value()
                .to_string()
                .trim_matches('"')
                .trim()
                .to_string()
        })
    };
    let make = get(exif::Tag::Make).unwrap_or_default();
    let model = get(exif::Tag::Model).unwrap_or_default();

    let camera = if model.is_empty() {
        if make.is_empty() {
            None
        } else {
            Some(make)
        }
    } else if !make.is_empty() && !model.to_lowercase().starts_with(&make.to_lowercase()) {
        Some(format!("{} {}", make, model))
    } else {
        Some(model)
    };

    (taken, camera, read_gps(&exif))
}

// ------------------------------------------------------------------- Doc chinh

/// Doc metadata cua mot file. Chi goi khi tieu chi thuc su can (lazy).
/// Khong bao gio panic - loi doc thi tra ve gia tri rong.
pub fn probe(path: &Path) -> MediaInfo {
    let mut info = MediaInfo::default();

    let mut head = [0u8; 16];
    let n = match File::open(path).and_then(|mut f| {
        f.seek(SeekFrom::Start(0))?;
        f.read(&mut head)
    }) {
        Ok(n) => n,
        Err(_) => return info,
    };

    if let Some((ext, kind)) = sniff(&head[..n]) {
        info.real_ext = Some(ext.to_string());
        info.real_kind = Some(kind.to_string());

        if kind == "image" {
            if let Ok(sz) = imagesize::size(path) {
                info.width = Some(sz.width as u32);
                info.height = Some(sz.height as u32);
            }
            if ext == "jpg" || ext == "heic" {
                let (taken, camera, gps) = read_exif(path);
                info.taken_ms = taken;
                info.camera = camera;
                info.place = gps.and_then(|(lat, lon)| {
                    crate::geo::nearest_place(lat, lon).map(|s| s.to_string())
                });
            }
        }
    }
    info
}

/// Doc Alternate Data Stream `Zone.Identifier` cua Windows
/// de biet file duoc tai ve tu ten mien nao.
pub fn download_source(path: &Path) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let ads = format!("{}:Zone.Identifier", path.to_string_lossy());
    let raw = std::fs::read_to_string(&ads).ok()?;
    let line = raw
        .lines()
        .find(|l| l.trim_start().to_lowercase().starts_with("hosturl="))
        .or_else(|| {
            raw.lines()
                .find(|l| l.trim_start().to_lowercase().starts_with("referrerurl="))
        })?;
    let url = line.split_once('=')?.1.trim();
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme
        .split(['/', ':', '?'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase();
    if host.is_empty() || !host.contains('.') {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_basic() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().0, "jpg");
        assert_eq!(sniff(b"%PDF-1.7").unwrap().0, "pdf");
        assert_eq!(sniff(b"MZ\x90\x00").unwrap().0, "exe");
        assert!(sniff(b"hello").is_none());
    }

    #[test]
    fn sniff_riff() {
        let mut b = Vec::from(*b"RIFF");
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(b"WEBP");
        assert_eq!(sniff(&b).unwrap().0, "webp");
    }

    /// Dựng một JPEG tối thiểu có khối EXIF chứa toạ độ GPS, để test TRỌN đường
    /// đọc GPS thật chứ không chỉ phần tính. Không dùng thư viện ghi EXIF (không
    /// có sẵn), nên viết thẳng byte theo chuẩn TIFF/EXIF little-endian.
    fn jpeg_voi_gps(lat_dms: [(u32, u32); 3], lat_ref: u8, lon_dms: [(u32, u32); 3], lon_ref: u8) -> Vec<u8> {
        let mut tiff: Vec<u8> = Vec::new();
        let u16le = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
        let u32le = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());

        tiff.extend_from_slice(b"II");        // little-endian
        u16le(&mut tiff, 42);
        u32le(&mut tiff, 8);                  // IFD0 tại offset 8
        // IFD0: 1 entry trỏ tới GPS IFD
        u16le(&mut tiff, 1);
        u16le(&mut tiff, 0x8825);             // GPS IFD pointer
        u16le(&mut tiff, 4);                  // type LONG
        u32le(&mut tiff, 1);
        u32le(&mut tiff, 26);                 // GPS IFD tại offset 26
        u32le(&mut tiff, 0);                  // hết IFD

        // GPS IFD: 4 entry
        let lat_off = 80u32;
        let lon_off = 104u32;
        u16le(&mut tiff, 4);
        // GPSLatitudeRef
        u16le(&mut tiff, 0x0001); u16le(&mut tiff, 2); u32le(&mut tiff, 2);
        tiff.extend_from_slice(&[lat_ref, 0, 0, 0]);
        // GPSLatitude
        u16le(&mut tiff, 0x0002); u16le(&mut tiff, 5); u32le(&mut tiff, 3); u32le(&mut tiff, lat_off);
        // GPSLongitudeRef
        u16le(&mut tiff, 0x0003); u16le(&mut tiff, 2); u32le(&mut tiff, 2);
        tiff.extend_from_slice(&[lon_ref, 0, 0, 0]);
        // GPSLongitude
        u16le(&mut tiff, 0x0004); u16le(&mut tiff, 5); u32le(&mut tiff, 3); u32le(&mut tiff, lon_off);
        u32le(&mut tiff, 0);                  // hết IFD

        // Dữ liệu rational (offset 80 và 104)
        assert_eq!(tiff.len(), 80, "offset vĩ độ phải là 80");
        for (n, d) in lat_dms { u32le(&mut tiff, n); u32le(&mut tiff, d); }
        assert_eq!(tiff.len(), 104, "offset kinh độ phải là 104");
        for (n, d) in lon_dms { u32le(&mut tiff, n); u32le(&mut tiff, d); }

        // Bọc thành JPEG: SOI + APP1(Exif) + EOI
        let mut jpg: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let payload_len = (6 + tiff.len() + 2) as u16; // "Exif\0\0" + tiff + 2 byte độ dài
        jpg.extend_from_slice(&payload_len.to_be_bytes());
        jpg.extend_from_slice(b"Exif\0\0");
        jpg.extend_from_slice(&tiff);
        jpg.extend_from_slice(&[0xFF, 0xD9]);
        jpg
    }

    #[test]
    fn doc_gps_trong_anh_ra_dung_noi_chup() {
        let dir = std::env::temp_dir().join(format!("foldu-gps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("anh-da-nang.jpg");

        // Đà Nẵng: 16°3'0" N, 108°13'12" E  ->  16.05, 108.22
        let bytes = jpeg_voi_gps(
            [(16, 1), (3, 1), (0, 1)], b'N',
            [(108, 1), (13, 1), (12, 1)], b'E',
        );
        std::fs::write(&path, &bytes).unwrap();

        let info = probe(&path);
        assert_eq!(info.place.as_deref(), Some("Da-Nang"), "phải đọc GPS ra Đà Nẵng, nhận: {:?}", info.place);

        // Ảnh nam bán cầu: Sydney -33.87, 151.21
        let path2 = dir.join("anh-sydney.jpg");
        let syd = jpeg_voi_gps(
            [(33, 1), (52, 1), (12, 1)], b'S',   // 33.87 nam
            [(151, 1), (12, 1), (36, 1)], b'E',  // 151.21 đông
        );
        std::fs::write(&path2, &syd).unwrap();
        assert_eq!(probe(&path2).place.as_deref(), Some("Sydney"), "vĩ độ Nam phải xử lý đúng dấu âm");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
