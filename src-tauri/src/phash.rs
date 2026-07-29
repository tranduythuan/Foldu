//! Tìm ảnh GẦN GIỐNG nhau, không phải giống hệt từng byte.
//!
//! Lọc trùng lặp ở `dedup.rs` chỉ bắt được file y hệt nhau. Nhưng trong thư viện
//! ảnh thật, thứ ăn dung lượng nhiều nhất lại là cùng một tấm ảnh lưu nhiều lần
//! ở kích thước hoặc mức nén khác nhau: bản gốc 4000px cạnh bản 800px đã thu nhỏ
//! để gửi đi, bản chụp màn hình cạnh bản tải về. Byte khác hẳn nhau nên hash thường vô dụng.
//!
//! Cách làm: băm tri giác kiểu dHash. Thu ảnh về 9x8 mức xám rồi so từng cặp điểm
//! cạnh nhau theo chiều ngang, được 64 bit mô tả "hình dáng sáng tối" của ảnh.
//! Hai ảnh cùng nội dung cho ra hai chuỗi bit lệch nhau rất ít, dù kích thước và
//! mức nén khác nhau.
//!
//! QUAN TRỌNG: ảnh gần giống KHÔNG phải ảnh giống hệt. Thuật toán có thể nhầm,
//! nên phần thừa chỉ bao giờ được dồn vào một thư mục để người dùng tự xem lại,
//! tuyệt đối không đưa vào Thùng rác. Ràng buộc đó nằm ở `planner.rs`.

use crate::scanner::FileEntry;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Đuôi file mà bộ giải mã đang bật có thể đọc
const DECODABLE: &[&str] = &["jpg", "jpeg", "png", "jfif"];

/// Quá số này thì bỏ qua, vì so từng cặp là O(n²).
/// 60.000 ảnh đã là 1,8 tỷ phép so, còn chấp nhận được; hơn nữa thì treo máy.
pub const MAX_IMAGES: usize = 60_000;

pub fn is_hashable(f: &FileEntry) -> bool {
    !f.is_dir && f.size > 0 && DECODABLE.contains(&f.ext.as_str())
}

/// Băm tri giác 64 bit. Trả về kèm khung ảnh để còn loại các cặp khác hẳn tỷ lệ.
pub fn dhash(path: &Path) -> Option<(u64, u32, u32)> {
    let img = image::open(path).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }

    // 9 cột để so được 8 cặp cạnh nhau trên mỗi hàng, 8 hàng -> đúng 64 bit
    let small = img
        .grayscale()
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_luma8();

    let mut hash = 0u64;
    let mut bit = 0u32;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            if left > right {
                hash |= 1u64 << bit;
            }
            bit += 1;
        }
    }
    Some((hash, w, h))
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Ảnh cùng nội dung thì tỷ lệ khung gần như không đổi dù thu phóng cỡ nào.
/// Chặn theo tỷ lệ giúp loại bớt các cặp trùng bit ngẫu nhiên.
fn similar_shape(a: (u32, u32), b: (u32, u32)) -> bool {
    let ra = a.0 as f64 / a.1 as f64;
    let rb = b.0 as f64 / b.1 as f64;
    (ra - rb).abs() / ra.max(rb) < 0.12
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearMember {
    pub id: u32,
    pub path: PathBuf,
    pub size: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearGroup {
    /// Bản giữ lại: ảnh nét nhất, hoà thì lấy file nặng hơn
    pub keeper: NearMember,
    pub extras: Vec<NearMember>,
    /// Dung lượng thu hồi được nếu bỏ hết phần thừa
    pub wasted: u64,
    /// Mức lệch lớn nhất trong nhóm, càng nhỏ càng chắc là cùng một ảnh
    pub max_distance: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NearReport {
    pub groups: Vec<NearGroup>,
    pub total_groups: usize,
    pub total_extras: usize,
    pub total_wasted: u64,
    pub hashed: usize,
    /// Bỏ qua vì quá nhiều ảnh
    pub skipped_too_many: bool,
    pub elapsed_ms: u64,
}

// ─────────────────────────────────────────────────── Gom nhóm bằng hợp-tìm

struct DisjointSet(Vec<usize>);

impl DisjointSet {
    fn new(n: usize) -> Self {
        DisjointSet((0..n).collect())
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut c = x;
        while self.0[c] != c {
            let next = self.0[c];
            self.0[c] = r;
            c = next;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0[rb] = ra;
        }
    }
}

/// `threshold` là số bit được phép lệch. 0 là y hệt, khoảng 8-12 bắt được ảnh
/// cùng nội dung khác kích thước, trên 16 bắt đầu nhầm sang ảnh khác.
///
/// `skip` chứa id các file đã nằm trong nhóm trùng lặp tuyệt đối, khỏi báo hai lần.
pub fn find_near_duplicate_images<F>(
    files: &[FileEntry],
    threshold: u32,
    skip: &std::collections::HashSet<u32>,
    mut on_progress: F,
) -> NearReport
where
    F: FnMut(usize, usize),
{
    let start = std::time::Instant::now();

    let targets: Vec<&FileEntry> = files
        .iter()
        .filter(|f| is_hashable(f) && !skip.contains(&f.id))
        .collect();

    if targets.len() < 2 {
        return NearReport {
            elapsed_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
    }
    if targets.len() > MAX_IMAGES {
        return NearReport {
            skipped_too_many: true,
            elapsed_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
    }

    on_progress(0, targets.len());
    let hashed: Vec<(&FileEntry, u64, u32, u32)> = targets
        .par_iter()
        .filter_map(|f| dhash(&f.path).map(|(h, w, ht)| (*f, h, w, ht)))
        .collect();
    on_progress(hashed.len(), targets.len());

    if hashed.len() < 2 {
        return NearReport {
            hashed: hashed.len(),
            elapsed_ms: start.elapsed().as_millis() as u64,
            ..Default::default()
        };
    }

    // So từng cặp. Chia việc theo hàng để mỗi luồng lo một dải i.
    let n = hashed.len();
    // Mượn chứ không chuyển quyền sở hữu: closure của rayon dùng `move`, mà bảng
    // băm còn phải dùng lại ở bước gom nhóm bên dưới.
    let hh = &hashed;
    let pairs: Vec<(usize, usize, u32)> = (0..n)
        .into_par_iter()
        .flat_map_iter(|i| {
            let (_, hi, wi, hti) = hh[i];
            (i + 1..n).filter_map(move |j| {
                let (_, hj, wj, htj) = hh[j];
                let d = hamming(hi, hj);
                if d <= threshold && similar_shape((wi, hti), (wj, htj)) {
                    Some((i, j, d))
                } else {
                    None
                }
            })
        })
        .collect();

    let mut ds = DisjointSet::new(n);
    let mut worst: HashMap<usize, u32> = HashMap::new();
    for (i, j, d) in &pairs {
        ds.union(*i, *j);
        let _ = d;
    }

    let mut buckets: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = ds.find(i);
        buckets.entry(r).or_default().push(i);
    }
    for (i, j, d) in &pairs {
        let r = ds.find(*i);
        let e = worst.entry(r).or_insert(0);
        if *d > *e {
            *e = *d;
        }
        let _ = j;
    }

    let mut groups = Vec::new();
    let mut total_extras = 0usize;
    let mut total_wasted = 0u64;

    for (root, mut idxs) in buckets {
        if idxs.len() < 2 {
            continue;
        }
        // Giữ ảnh nhiều điểm ảnh nhất; hoà thì giữ file nặng hơn (ít nén hơn)
        idxs.sort_by_key(|&i| {
            let (f, _, w, h) = hashed[i];
            (
                std::cmp::Reverse((w as u64) * (h as u64)),
                std::cmp::Reverse(f.size),
                f.path.as_os_str().len(),
            )
        });

        let mk = |i: usize| {
            let (f, _, w, h) = hashed[i];
            NearMember {
                id: f.id,
                path: f.path.clone(),
                size: f.size,
                width: w,
                height: h,
            }
        };
        let keeper = mk(idxs[0]);
        let extras: Vec<NearMember> = idxs[1..].iter().map(|&i| mk(i)).collect();
        let wasted: u64 = extras.iter().map(|m| m.size).sum();

        total_extras += extras.len();
        total_wasted += wasted;
        groups.push(NearGroup {
            keeper,
            extras,
            wasted,
            max_distance: *worst.get(&root).unwrap_or(&0),
        });
    }

    groups.sort_by(|a, b| b.wasted.cmp(&a.wasted));

    NearReport {
        total_groups: groups.len(),
        groups,
        total_extras,
        total_wasted,
        hashed: hashed.len(),
        skipped_too_many: false,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_dem_dung_so_bit_lech() {
        assert_eq!(hamming(0, 0), 0);
        assert_eq!(hamming(0b1011, 0b1001), 1);
        assert_eq!(hamming(u64::MAX, 0), 64);
    }

    #[test]
    fn ty_le_khung_lech_nhieu_thi_khong_coi_la_mot_cap() {
        assert!(similar_shape((4000, 3000), (800, 600)));   // cùng 4:3
        assert!(similar_shape((1920, 1080), (1280, 720)));  // cùng 16:9
        assert!(!similar_shape((4000, 3000), (1080, 1920))); // ngang với dọc
        assert!(!similar_shape((1000, 1000), (1000, 500))); // vuông với 2:1
    }

    #[test]
    fn hop_tim_gom_dung_cac_phan_tu_lien_thong() {
        let mut ds = DisjointSet::new(6);
        ds.union(0, 1);
        ds.union(1, 2);
        ds.union(4, 5);
        assert_eq!(ds.find(0), ds.find(2));
        assert_ne!(ds.find(0), ds.find(3));
        assert_eq!(ds.find(4), ds.find(5));
    }

    // ───────────────────────── Kiểm bằng ảnh thật, không phải dữ liệu giả ────

    /// Vẽ một tấm ảnh có hình thù rõ ràng để băm tri giác bám được.
    /// `seed` đổi thì nội dung đổi hẳn.
    fn ve_anh(w: u32, h: u32, seed: u32) -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            // Vài dải chéo cộng một khối đặc, đủ tương phản để dHash phân biệt
            let soc = ((fx * 6.0 + fy * 3.0 + seed as f32).sin() * 0.5 + 0.5) * 255.0;
            let khoi = if fx > 0.55 && fx < 0.8 && fy > 0.2 && fy < 0.5 { 40.0 } else { 0.0 };
            let v = (soc - khoi).clamp(0.0, 255.0) as u8;
            image::Rgb([v, v.wrapping_add(seed as u8 * 20), 255 - v])
        })
    }

    struct ThuMucTam(std::path::PathBuf);
    impl ThuMucTam {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "foldu-phash-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&p).unwrap();
            ThuMucTam(p)
        }
        /// Lưu ảnh ra JPEG và trả về FileEntry như bộ quét sẽ tạo
        fn luu(&self, id: u32, ten: &str, img: &image::RgbImage) -> FileEntry {
            let path = self.0.join(ten);
            img.save(&path).unwrap();
            let size = std::fs::metadata(&path).unwrap().len();
            FileEntry {
                id,
                path: path.clone(),
                name: ten.to_string(),
                root: self.0.clone(),
                is_dir: false,
                project_marker: None,
                ext: crate::util::ext_of(ten),
                size,
                mtime: 0,
                ctime: 0,
                atime: 0,
                parent: self.0.clone(),
                depth: 0,
            }
        }
    }
    impl Drop for ThuMucTam {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn cung_mot_anh_khac_kich_thuoc_thi_bam_ra_gan_giong_nhau() {
        let d = ThuMucTam::new("scale");
        let goc = ve_anh(1200, 900, 1);
        let nho = image::imageops::resize(&goc, 400, 300, image::imageops::FilterType::Lanczos3);
        let khac = ve_anh(1200, 900, 5);

        let p_goc = d.luu(0, "goc.jpg", &goc).path;
        let p_nho = d.luu(1, "nho.jpg", &nho).path;
        let p_khac = d.luu(2, "khac.jpg", &khac).path;

        let (h_goc, _, _) = dhash(&p_goc).expect("băm được ảnh gốc");
        let (h_nho, _, _) = dhash(&p_nho).expect("băm được ảnh thu nhỏ");
        let (h_khac, _, _) = dhash(&p_khac).expect("băm được ảnh khác");

        let gan = hamming(h_goc, h_nho);
        let xa = hamming(h_goc, h_khac);
        eprintln!("\n  Lệch giữa bản gốc và bản thu nhỏ: {} bit", gan);
        eprintln!("  Lệch giữa bản gốc và ảnh khác   : {} bit\n", xa);

        assert!(gan <= 10, "cùng một ảnh khác kích thước phải lệch ít, thực tế {} bit", gan);
        assert!(xa > gan + 5, "ảnh khác hẳn phải lệch nhiều hơn rõ rệt: {} so với {}", xa, gan);
    }

    #[test]
    fn gom_dung_nhom_va_giu_lai_ban_net_nhat() {
        let d = ThuMucTam::new("group");
        let a = ve_anh(1600, 1200, 2);
        let files = vec![
            d.luu(0, "a-nho.jpg", &image::imageops::resize(&a, 400, 300, image::imageops::FilterType::Lanczos3)),
            d.luu(1, "a-goc.jpg", &a),
            d.luu(2, "a-vua.jpg", &image::imageops::resize(&a, 800, 600, image::imageops::FilterType::Lanczos3)),
            d.luu(3, "b.jpg", &ve_anh(1600, 1200, 9)),
        ];

        let r = find_near_duplicate_images(&files, 10, &std::collections::HashSet::new(), |_, _| {});
        eprintln!("\n  Số nhóm: {}, số bản thừa: {}\n", r.total_groups, r.total_extras);

        assert_eq!(r.total_groups, 1, "ba bản của cùng một ảnh phải gom thành đúng một nhóm");
        assert_eq!(r.total_extras, 2, "giữ một bản, hai bản còn lại là thừa");

        let g = &r.groups[0];
        assert_eq!(g.keeper.id, 1, "phải giữ bản nét nhất (1600x1200)");
        assert_eq!(g.keeper.width, 1600);
        assert!(!g.extras.iter().any(|e| e.id == 3), "ảnh khác không được lôi vào nhóm");
        assert!(g.wasted > 0);
    }

    #[test]
    fn bo_qua_nhung_file_da_nam_trong_nhom_trung_tuyet_doi() {
        let d = ThuMucTam::new("skip");
        let a = ve_anh(1000, 750, 3);
        let files = vec![
            d.luu(0, "x.jpg", &a),
            d.luu(1, "y.jpg", &image::imageops::resize(&a, 500, 375, image::imageops::FilterType::Lanczos3)),
        ];

        let mut bo = std::collections::HashSet::new();
        bo.insert(1u32);
        let r = find_near_duplicate_images(&files, 10, &bo, |_, _| {});
        assert_eq!(r.total_groups, 0, "file đã bị lọc trùng tuyệt đối bắt rồi thì không báo lại");
    }

    #[test]
    fn anh_ngang_va_anh_doc_khong_bao_gio_gom_chung() {
        let d = ThuMucTam::new("shape");
        let ngang = ve_anh(1200, 600, 4);
        let doc = image::imageops::rotate90(&ngang);
        let files = vec![d.luu(0, "ngang.jpg", &ngang), d.luu(1, "doc.jpg", &doc)];

        let r = find_near_duplicate_images(&files, 20, &std::collections::HashSet::new(), |_, _| {});
        assert_eq!(r.total_groups, 0, "khác hẳn tỷ lệ khung thì không phải một cặp");
    }
}
