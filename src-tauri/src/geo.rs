//! Tra toạ độ GPS ra tên nơi chốn, hoàn toàn offline, phủ toàn thế giới.
//!
//! Ảnh chụp bằng điện thoại có bật định vị ghi sẵn toạ độ vào EXIF. Toạ độ thô
//! (16.05, 108.22) thì vô nghĩa với người dùng — họ muốn thấy "Da-Nang".
//!
//! Dữ liệu: bảng ~34.000 thành phố (dân số trên 15.000) của GeoNames, nhúng thẳng
//! trong file chạy dưới dạng nhị phân gọn (~730KB). Không cần mạng. Nguồn:
//! GeoNames (https://www.geonames.org), giấy phép CC BY 4.0.
//!
//! Hai chốt quan trọng để tra ra tên ĐÚNG Ý người dùng, không phải điểm gần nhất máy móc:
//!   1. Ưu tiên THÀNH PHỐ LỚN NHẤT trong bán kính gần (30km). Nếu chỉ lấy điểm gần
//!      nhất thì ảnh giữa Hà Nội ra tên một phường ("Yen-Phu") thay vì "Hanoi",
//!      vì phường ở sát hơn trung tâm thành phố.
//!   2. Xa mọi thành phố hơn `MAX_KM` thì KHÔNG đoán bừa, trả về None -> file vào
//!      "Khong-Ro-Noi-Chup". Thà nói không biết còn hơn dán nhãn sai.

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Trong bán kính này thì ưu tiên thành phố đông dân nhất (coi như "khu vực thành phố")
const CLOSE_KM: f64 = 30.0;
/// Xa hơn mức này thì coi như không biết nơi chụp
const MAX_KM: f64 = 150.0;

const RAW: &[u8] = include_bytes!("cities.bin");

struct Db {
    lat: Vec<f32>,
    lon: Vec<f32>,
    pop: Vec<u32>,
    name: Vec<String>,
    /// Ô lưới 1°×1° -> danh sách chỉ số thành phố, để không phải quét cả 34k điểm
    grid: HashMap<(i16, i16), Vec<u32>>,
}

static DB: Lazy<Db> = Lazy::new(parse);

fn parse() -> Db {
    let mut lat = Vec::new();
    let mut lon = Vec::new();
    let mut pop = Vec::new();
    let mut name = Vec::new();
    let mut grid: HashMap<(i16, i16), Vec<u32>> = HashMap::new();

    if RAW.len() < 4 {
        return Db { lat, lon, pop, name, grid };
    }
    let count = u32::from_le_bytes([RAW[0], RAW[1], RAW[2], RAW[3]]) as usize;
    let mut p = 4usize;
    for _ in 0..count {
        if p + 13 > RAW.len() {
            break;
        }
        let la = f32::from_le_bytes([RAW[p], RAW[p + 1], RAW[p + 2], RAW[p + 3]]);
        let lo = f32::from_le_bytes([RAW[p + 4], RAW[p + 5], RAW[p + 6], RAW[p + 7]]);
        let pp = u32::from_le_bytes([RAW[p + 8], RAW[p + 9], RAW[p + 10], RAW[p + 11]]);
        let len = RAW[p + 12] as usize;
        p += 13;
        if p + len > RAW.len() {
            break;
        }
        let nm = String::from_utf8_lossy(&RAW[p..p + len]).into_owned();
        p += len;

        let idx = lat.len() as u32;
        grid.entry((la.floor() as i16, lo.floor() as i16))
            .or_default()
            .push(idx);
        lat.push(la);
        lon.push(lo);
        pop.push(pp);
        name.push(nm);
    }
    Db { lat, lon, pop, name, grid }
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

/// Tên nơi chốn hợp lý nhất cho toạ độ, hoặc None nếu chẳng có thành phố nào đủ gần.
pub fn nearest_place(lat: f64, lon: f64) -> Option<&'static str> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let db = &*DB;
    if db.lat.is_empty() {
        return None;
    }

    // Ô lưới cần quét: vĩ độ ±2 (≈220km), kinh độ mở rộng theo vĩ độ vì gần cực thì
    // 1° kinh ngắn lại. cos(lat) tránh chia 0 ở gần cực.
    let base = (lat.floor() as i16, lon.floor() as i16);
    let cos = lat.to_radians().cos().abs().max(0.02);
    let lon_span = ((MAX_KM / (111.0 * cos)).ceil() as i16).clamp(1, 30);

    let mut best_close: Option<(u32, f64, u32)> = None; // (pop, dist, idx) trong CLOSE_KM
    let mut best_far: Option<(f64, u32)> = None; // (dist, idx) trong MAX_KM

    for dla in -2i16..=2 {
        for dlo in -lon_span..=lon_span {
            let key_lon = (((base.1 as i32 + dlo as i32 + 180).rem_euclid(360)) - 180) as i16;
            let key = (base.0 + dla, key_lon);
            let Some(ids) = db.grid.get(&key) else { continue };
            for &i in ids {
                let iu = i as usize;
                let d = haversine_km(lat, lon, db.lat[iu] as f64, db.lon[iu] as f64);
                if d <= CLOSE_KM {
                    // Trong bán kính gần: chọn đông dân nhất, hoà thì gần hơn
                    let better = match best_close {
                        None => true,
                        Some((bp, bd, _)) => db.pop[iu] > bp || (db.pop[iu] == bp && d < bd),
                    };
                    if better {
                        best_close = Some((db.pop[iu], d, i));
                    }
                }
                if d <= MAX_KM {
                    if best_far.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best_far = Some((d, i));
                    }
                }
            }
        }
    }

    let idx = best_close
        .map(|(_, _, i)| i)
        .or(best_far.map(|(_, i)| i))?;
    Some(&db.name[idx as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn khoang_cach_hop_ly() {
        let d = haversine_km(21.03, 105.85, 10.78, 106.70);
        assert!((1100.0..1200.0).contains(&d), "khoảng cách HN-HCM sai: {}", d);
    }

    #[test]
    fn bang_du_lieu_nap_duoc() {
        assert!(DB.lat.len() > 30000, "phải nạp được vài chục nghìn thành phố, có {}", DB.lat.len());
    }

    #[test]
    fn thanh_pho_lon_tra_dung_ten_khong_ra_ten_phuong() {
        // Giữa Hà Nội: phải ra "Hanoi" chứ không phải tên một phường sát hơn
        assert_eq!(nearest_place(21.03, 105.85), Some("Hanoi"));
        assert_eq!(nearest_place(16.05, 108.22), Some("Da-Nang"));
        assert_eq!(nearest_place(35.68, 139.65), Some("Tokyo"));
        assert_eq!(nearest_place(13.75, 100.50), Some("Bangkok"));
    }

    #[test]
    fn nguoi_nuoc_ngoai_cung_dung_duoc() {
        // Đây là cái bảng cũ (85 thành phố) làm không nổi
        assert_eq!(nearest_place(48.137, 11.575), Some("Munich"));
        assert_eq!(nearest_place(53.481, -2.237), Some("Manchester"));
        // Austin, Texas (không phải Austin, Minnesota) — phân biệt nhờ toạ độ
        assert_eq!(nearest_place(30.267, -97.743), Some("Austin"));
        // Nam bán cầu
        assert_eq!(nearest_place(-33.868, 151.207), Some("Sydney"));
    }

    #[test]
    fn tinh_le_viet_nam_ra_dung_tinh() {
        assert_eq!(nearest_place(21.723, 104.911), Some("Yen-Bai"));
        assert_eq!(nearest_place(11.946, 108.442), Some("Da-Lat"));
    }

    #[test]
    fn xa_moi_thanh_pho_thi_khong_doan_bua() {
        // Giữa Thái Bình Dương, không thành phố nào trong 150km
        assert_eq!(nearest_place(0.0, -140.0), None);
    }

    #[test]
    fn toa_do_vo_ly_tra_none() {
        assert_eq!(nearest_place(200.0, 0.0), None);
        assert_eq!(nearest_place(0.0, 999.0), None);
    }

    #[test]
    fn moi_ten_deu_la_ascii_hop_le_lam_ten_thu_muc() {
        for n in &DB.name {
            assert!(n.is_ascii() && !n.is_empty(), "tên hỏng: {:?}", n);
            for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
                assert!(!n.contains(c), "tên chứa ký tự cấm: {}", n);
            }
            assert!(!n.ends_with('.') && !n.ends_with(' '));
        }
    }
}
