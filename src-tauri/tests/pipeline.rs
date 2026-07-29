//! Test tich hop: chay TRON luong quet -> lap ke hoach -> thuc thi -> hoan tac
//! tren mot thu muc that, doi chieu voi cac tieu chi nghiem thu o §14 cua ban dac ta.

use foldu_lib::config::{DupAction, Mode, Profile, Settings};
use foldu_lib::executor;
use foldu_lib::planner::{self, OpAction, Plan};
use foldu_lib::scanner::Scanner;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// ─────────────────────────────────────────────────────────────────── Tien ich

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "foldu-it-{}-{}-{}",
            tag,
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Fixture { root }
    }

    fn file(&self, rel: &str, content: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        p
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn profile(layers: &[&str]) -> Profile {
    let mut p = Profile::default();
    p.layers = layers.iter().map(|s| s.to_string()).collect();
    p.duplicates.enabled = false;
    p
}

fn run_plan(root: &Path, prof: &Profile) -> (Vec<foldu_lib::scanner::FileEntry>, Plan) {
    let settings = Settings::default();
    let roots = vec![root.to_path_buf()];
    let scan = Scanner::new(prof, vec![]).run(&roots, |_, _| {});
    let plan = planner::build_plan(&scan.files, prof, &settings, &roots, |_, _, _| {});
    (scan.files, plan)
}

fn apply(plan: &Plan) -> executor::ExecResult {
    executor::execute(
        plan,
        "test",
        &HashSet::new(),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("thuc thi that bai")
}

// ══════════════════════════════════════════════════════════ 1. Preview = thực tế

#[test]
fn preview_khop_100_phan_tram_voi_ket_qua_that() {
    let fx = Fixture::new("preview");
    fx.file("Báo cáo tháng 10.pdf", "a");
    fx.file("anh.jpg", "b");
    fx.file("bang luong.xlsx", "c");
    fx.file("ghi chu.txt", "d");

    let prof = profile(&["TYPE"]);
    let (_, plan) = run_plan(&fx.root, &prof);

    // Ghi lai dich da hua trong ban xem truoc
    let promised: Vec<(PathBuf, PathBuf)> = plan
        .ops
        .iter()
        .filter(|o| o.action == OpAction::Move)
        .map(|o| (o.src.clone(), o.dest.clone()))
        .collect();
    assert!(!promised.is_empty(), "phai co thao tac de kiem tra");

    let res = apply(&plan);
    assert_eq!(res.failed, 0, "khong duoc co loi nao");

    for (src, dest) in &promised {
        assert!(dest.exists(), "file phai co mat dung cho da hua: {:?}", dest);
        assert!(!src.exists(), "file khong duoc con o cho cu: {:?}", src);
    }
    assert_eq!(res.done, promised.len());
}

// ══════════════════════════════════════════════ 2. Không bao giờ ghi đè dữ liệu

#[test]
fn trung_ten_khac_noi_dung_thi_doi_ten_chu_khong_ghi_de() {
    let fx = Fixture::new("collide");
    fx.file("a.txt", "NOI DUNG MOT");
    fx.file("thu-muc-con/a.txt", "NOI DUNG HAI");

    let prof = profile(&["TYPE"]);
    let (_, plan) = run_plan(&fx.root, &prof);

    let dests: Vec<&PathBuf> = plan.ops.iter().map(|o| &o.dest).collect();
    let unique: HashSet<String> = dests
        .iter()
        .map(|d| d.to_string_lossy().to_lowercase())
        .collect();
    assert_eq!(unique.len(), dests.len(), "hai file khong duoc trung dich");

    apply(&plan);

    // Ca hai noi dung deu phai con nguyen ven
    let mut found: Vec<String> = Vec::new();
    for e in walk(&fx.root) {
        if e.extension().map(|x| x == "txt").unwrap_or(false) {
            found.push(fs::read_to_string(&e).unwrap());
        }
    }
    found.sort();
    assert_eq!(found, vec!["NOI DUNG HAI", "NOI DUNG MOT"], "khong duoc mat noi dung nao");
}

// ═══════════════════════════════════════════ 3. Hoàn tác trả về 100% + timestamp

#[test]
fn hoan_tac_tra_ve_dung_cho_cu_va_giu_nguyen_dau_thoi_gian() {
    let fx = Fixture::new("undo");
    let paths = vec![
        fx.file("Hợp đồng ABC.docx", "x"),
        fx.file("anh nghi mat.jpg", "y"),
        fx.file("thu-muc-con/bao cao.pdf", "z"),
    ];

    // Dat mot moc thoi gian ro rang de con doi chieu
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    for p in &paths {
        let f = fs::OpenOptions::new().write(true).open(p).unwrap();
        f.set_times(fs::FileTimes::new().set_modified(stamp)).unwrap();
    }

    let prof = profile(&["TYPE", "TIME_MODIFIED:%Y"]);
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);
    assert_eq!(res.failed, 0);
    for p in &paths {
        assert!(!p.exists(), "file phai da roi cho cu");
    }

    let undo = executor::undo_session(&res.session, None, |_, _| {}).expect("hoan tac that bai");

    assert_eq!(undo.failed, 0, "hoan tac khong duoc that bai");
    assert!(undo.conflicts.is_empty(), "khong duoc co xung dot: {:?}", undo.conflicts);
    assert_eq!(undo.restored, res.done, "phai tra ve dung so file da di chuyen");

    for p in &paths {
        assert!(p.exists(), "file phai tro ve dung cho cu: {:?}", p);
        let m = fs::metadata(p).unwrap().modified().unwrap();
        let diff = m
            .duration_since(stamp)
            .or_else(|_| stamp.duration_since(m))
            .unwrap();
        assert!(diff < Duration::from_secs(2), "dau thoi gian phai duoc giu nguyen");
    }
    assert!(undo.removed_dirs > 0, "phai don sach thu muc rong do minh tao ra");
}

// ═════════════════════════════════════ 4. Hoàn tác chỉ xoá thư mục DO MÌNH tạo

#[test]
fn hoan_tac_khong_dung_vao_thu_muc_rong_von_co_tu_truoc() {
    let fx = Fixture::new("emptydir");
    fx.file("tai lieu.pdf", "a");
    let pre_existing = fx.root.join("thu-muc-rong-cua-toi");
    fs::create_dir_all(&pre_existing).unwrap();

    let prof = profile(&["TYPE"]);
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);
    executor::undo_session(&res.session, None, |_, _| {}).unwrap();

    assert!(
        pre_existing.exists(),
        "thu muc rong von co tu truoc KHONG duoc bi xoa"
    );
}

// ══════════════════════════════════════════════ 5. Giữ file đi kèm cùng nhau

#[test]
fn cap_raw_va_jpg_luon_nam_chung_mot_thu_muc() {
    let fx = Fixture::new("sidecar");
    // Cung ten goc, hai duoi khac nhau -> phai di cung nhau
    fx.file("IMG_0001.cr2", &"R".repeat(4096));
    fx.file("IMG_0001.jpg", "J");
    // Them file de tang so luong
    fx.file("ghi chu.txt", "n");

    // TYPE se tach RAW va JPG ra hai nhom khac nhau NEU khong co rang buoc sidecar
    let mut prof = profile(&["TYPE"]);
    prof.safety.keep_sidecar_together = true;
    let (_, plan) = run_plan(&fx.root, &prof);

    let dir_of = |name: &str| -> PathBuf {
        plan.ops
            .iter()
            .find(|o| o.src.file_name().unwrap() == name)
            .unwrap_or_else(|| panic!("khong tim thay {}", name))
            .dest
            .parent()
            .unwrap()
            .to_path_buf()
    };
    assert_eq!(
        dir_of("IMG_0001.cr2"),
        dir_of("IMG_0001.jpg"),
        "RAW va JPG cung ten goc phai nam chung thu muc"
    );
}

#[test]
fn tat_rang_buoc_sidecar_thi_raw_va_jpg_bi_tach() {
    let fx = Fixture::new("nosidecar");
    fx.file("IMG_0001.cr2", &"R".repeat(4096));
    fx.file("IMG_0001.jpg", "J");

    let mut prof = profile(&["TYPE"]);
    prof.safety.keep_sidecar_together = false;
    let (_, plan) = run_plan(&fx.root, &prof);

    let dir_of = |name: &str| -> PathBuf {
        plan.ops
            .iter()
            .find(|o| o.src.file_name().unwrap() == name)
            .unwrap()
            .dest
            .parent()
            .unwrap()
            .to_path_buf()
    };
    assert_ne!(dir_of("IMG_0001.cr2"), dir_of("IMG_0001.jpg"));
}

// ═══════════════════════════════════════════ 6. Không phá thư mục dự án

#[test]
fn thu_muc_du_an_duoc_di_chuyen_nguyen_khoi() {
    let fx = Fixture::new("project");
    fx.file("du-an-web/package.json", "{}");
    fx.file("du-an-web/index.js", "console.log(1)");
    fx.file("du-an-web/src/app.js", "x");
    fx.file("tai lieu.pdf", "p");

    let prof = profile(&["TYPE"]);
    let (files, plan) = run_plan(&fx.root, &prof);

    // Ca thu muc du an phai la MOT muc duy nhat, khong phai 3 file roi
    let proj: Vec<_> = files.iter().filter(|f| f.is_dir).collect();
    assert_eq!(proj.len(), 1, "phai nhan dien dung mot thu muc du an");
    assert_eq!(proj[0].project_marker.as_deref(), Some("package.json"));
    assert!(
        !files.iter().any(|f| f.name == "app.js"),
        "khong duoc duyet vao trong thu muc du an"
    );

    apply(&plan);

    // Sau khi di chuyen, cau truc ben trong phai con nguyen
    let moved = plan
        .ops
        .iter()
        .find(|o| o.is_dir)
        .expect("phai co thao tac cho thu muc du an");
    assert!(moved.dest.join("package.json").exists());
    assert!(moved.dest.join("src/app.js").exists());
}

// ══════════════════════════════════════════════════ 7. Dò trùng lặp 3 tầng

#[test]
fn phat_hien_dung_file_trung_noi_dung_va_bo_qua_file_khac_noi_dung() {
    let fx = Fixture::new("dup");
    let body = "NOI DUNG GIONG HET NHAU".repeat(500);
    fx.file("bao cao.pdf", &body);
    fx.file("luu tru/bao cao (ban sao).pdf", &body);
    fx.file("khac.pdf", &"KHAC HAN".repeat(500)); // cung kich thuoc? khong — khac
    fx.file("cung-co-nhung-khac.pdf", &body.replace("GIONG", "KHACX"));

    let mut prof = profile(&["TYPE"]);
    prof.duplicates.enabled = true;
    prof.duplicates.action = DupAction::Report;

    let (files, _) = run_plan(&fx.root, &prof);
    let report = foldu_lib::dedup::find_duplicates(&files, prof.duplicates.strategy, |_, _| {});

    assert_eq!(report.total_groups, 1, "chi co dung mot nhom trung lap");
    assert_eq!(report.total_extras, 1, "chi co mot ban thua");
    assert_eq!(report.groups[0].wasted, body.len() as u64);

    // Chien luoc mac dinh giu ban co duong dan NGAN nhat
    assert_eq!(
        report.groups[0].keeper.path.file_name().unwrap(),
        "bao cao.pdf"
    );
}

// ═══════════════════════════════════ 8. Chế độ COPY giữ nguyên bản gốc

#[test]
fn che_do_sao_chep_giu_nguyen_ban_goc() {
    let fx = Fixture::new("copy");
    let src = fx.file("anh.jpg", "abc");

    let mut prof = profile(&["TYPE"]);
    prof.mode = Mode::Copy;
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);

    assert_eq!(res.failed, 0);
    assert!(src.exists(), "ban goc phai con nguyen");
    let dest = &plan.ops.iter().find(|o| o.action == OpAction::Copy).unwrap().dest;
    assert!(dest.exists(), "ban sao phai duoc tao");

    // Hoan tac che do COPY = xoa ban sao, khong dung vao ban goc
    executor::undo_session(&res.session, None, |_, _| {}).unwrap();
    assert!(src.exists(), "hoan tac khong duoc dung vao ban goc");
    assert!(!dest.exists(), "hoan tac phai xoa ban sao");
}

// ═════════════════════════════ 9. Tên thư mục sinh ra luôn hợp lệ với Windows

#[test]
fn ten_thu_muc_sinh_ra_khong_bao_gio_chua_ky_tu_cam() {
    let fx = Fixture::new("sanitize");
    // "con" la TEN THIET BI CAM tren Windows. Neu khong lam sach, buoc tao thu muc
    // se that bai — day chinh la loai loi ban v1 khong he xu ly.
    for i in 1..=6 {
        fx.file(&format!("Con bao so {} - bien dong.pdf", i), "x");
    }
    for i in 1..=5 {
        fx.file(&format!("Aux thiet bi phu {}.txt", i), "y");
    }

    // Ep thang mot ten thiet bi cam va mot ten ket thuc bang dau cham + khoang trang
    let prof = profile(&["LITERAL:CON/aux. ", "AUTO_PROJECT"]);
    let (_, plan) = run_plan(&fx.root, &prof);

    let rel0 = plan.ops[0]
        .dest
        .strip_prefix(&fx.root)
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        rel0.starts_with("_CON\\_aux\\"),
        "ten thiet bi cam phai duoc lam sach, nhan duoc: {}",
        rel0
    );

    // Khong duoc ton tai doan thu muc mang ten thiet bi cam
    for o in &plan.ops {
        let rel = o.dest.strip_prefix(&fx.root).unwrap().to_string_lossy().to_string();
        let segs: Vec<&str> = rel.split('\\').collect();
        for seg in &segs[..segs.len().saturating_sub(1)] {
            let stem = seg.split('.').next().unwrap_or("").to_uppercase();
            assert!(
                !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL"),
                "sinh ra ten thiet bi cam: {}",
                rel
            );
        }
    }

    const FORBIDDEN: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
    for o in &plan.ops {
        let rel = o
            .dest
            .strip_prefix(&fx.root)
            .unwrap_or(&o.dest)
            .to_string_lossy()
            .to_string();
        // Bo qua phan ten file goc, chi kiem tra cac doan THU MUC do phan mem sinh
        let segs: Vec<&str> = rel.split('\\').collect();
        for seg in &segs[..segs.len().saturating_sub(1)] {
            for c in FORBIDDEN {
                assert!(
                    !seg.contains(*c),
                    "ten thu muc '{}' chua ky tu cam '{}'",
                    seg,
                    c
                );
            }
            assert!(!seg.ends_with('.') && !seg.ends_with(' '));
        }
    }
    // Va thuc te tao duoc that
    let res = apply(&plan);
    assert_eq!(res.failed, 0, "loi: {:?}", res.errors);
}

// ═════════════════════════════════════════ 10. Chế độ chỉ báo cáo không đụng gì

#[test]
fn che_do_chi_bao_cao_khong_dung_vao_bat_cu_thu_gi() {
    let fx = Fixture::new("report");
    let a = fx.file("anh.jpg", "1");
    let b = fx.file("tai lieu.pdf", "2");

    let mut prof = profile(&["TYPE"]);
    prof.mode = Mode::ReportOnly;
    let (_, plan) = run_plan(&fx.root, &prof);

    assert!(
        plan.ops.iter().all(|o| o.action == OpAction::Keep),
        "che do chi bao cao phai sinh toan thao tac Keep"
    );
    assert!(a.exists() && b.exists());
}

// ═══════════════════════════════ 11. Bản tiếng Anh chạy thật ═══════════════

/// Đổi sang tiếng Anh thì THƯ MỤC TẠO RA TRÊN Ổ ĐĨA phải đổi theo, không chỉ
/// chữ trên màn hình. Đây là chỗ dễ làm nửa vời nhất: dịch giao diện xong vẫn
/// đẻ ra `01-Hinh-Anh` thì người nước ngoài mở ổ đĩa vẫn không hiểu gì.
#[test]
fn doi_sang_tieng_anh_thi_ten_thu_muc_tren_o_dia_cung_doi() {
    let _g = foldu_lib::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fx = Fixture::new("en-folders");
    fx.file("holiday.jpg", "a");
    fx.file("report.pdf", "b");
    fx.file("song.mp3", "c");
    fx.file("Screenshot 2026-03-15.png", "d");

    let run_with = |lang| {
        foldu_lib::i18n::set_lang(lang);
        let settings = Settings::default(); // sinh lại bảng nhóm file theo ngôn ngữ
        let mut prof = profile(&["SCREENSHOT_DETECT", "TYPE"]);
        prof.recursive = true;
        let scan = Scanner::new(&prof, vec![]).run(&[fx.root.clone()], |_, _| {});
        let plan = planner::build_plan(&scan.files, &prof, &settings, &[fx.root.clone()], |_, _, _| {});
        let mut dirs: Vec<String> = plan
            .ops
            .iter()
            .filter_map(|o| o.dest.parent())
            .filter_map(|p| p.strip_prefix(&fx.root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    };

    let vi = run_with(foldu_lib::i18n::Lang::Vi);
    let en = run_with(foldu_lib::i18n::Lang::En);
    foldu_lib::i18n::set_lang(foldu_lib::i18n::Lang::Vi);

    eprintln!("\n  VI: {:?}\n  EN: {:?}\n", vi, en);

    assert!(vi.iter().any(|d| d.contains("01-Hinh-Anh")), "bản Việt phải ra 01-Hinh-Anh: {:?}", vi);
    assert!(en.iter().any(|d| d.contains("01-Images")), "bản Anh phải ra 01-Images: {:?}", en);
    assert!(vi.iter().any(|d| d.contains("Anh-Chup-Man-Hinh")), "{:?}", vi);
    assert!(en.iter().any(|d| d.contains("Screenshots")), "{:?}", en);
    assert!(
        !en.iter().any(|d| d.contains("Hinh-Anh") || d.contains("Tai-Lieu") || d.contains("Am-Thanh")),
        "bản Anh không được sót tên tiếng Việt: {:?}",
        en
    );
}

/// Nhận xét ở màn Phân tích và thông báo an toàn cũng phải ra tiếng Anh
#[test]
fn nhan_xet_va_thong_bao_cung_ra_tieng_anh() {
    let _g = foldu_lib::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fx = Fixture::new("en-msg");
    for i in 1..=12 {
        fx.file(&format!("IMG_{:04}.jpg", i), "x"); // tên vô nghĩa -> sinh nhận xét
    }

    foldu_lib::i18n::set_lang(foldu_lib::i18n::Lang::En);
    let settings = Settings::default();
    let prof = profile(&["TYPE"]);
    let scan = Scanner::new(&prof, vec![]).run(&[fx.root.clone()], |_, _| {});
    let an = foldu_lib::analytics::analyze(&scan, &settings, &[fx.root.clone()]);
    let notes = an.health_notes.join(" | ");

    let sys = foldu_lib::safety::system_drive();
    let blocked = foldu_lib::safety::check_source(&PathBuf::from(format!("{}:\\", sys)));
    foldu_lib::i18n::set_lang(foldu_lib::i18n::Lang::Vi);

    eprintln!("\n  EN notes: {}\n  EN block: {}\n", notes, blocked.reason);

    const VN_CHARS: &str = "àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ";
    for (what, s) in [("nhận xét", notes.as_str()), ("thông báo chặn", blocked.reason.as_str())] {
        assert!(!s.is_empty(), "{} không được rỗng", what);
        assert!(
            !s.chars().any(|c| VN_CHARS.contains(c)),
            "{} còn chữ tiếng Việt: {}",
            what,
            s
        );
    }
    assert!(blocked.reason.contains("Windows"));
}

/// Ngôn ngữ phải sống sót qua lần mở lại: ghi vào settings rồi đọc lại đúng
#[test]
fn ngon_ngu_duoc_ghi_nho_qua_lan_mo_lai() {
    let _g = foldu_lib::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use foldu_lib::i18n::Lang;

    foldu_lib::i18n::set_lang(Lang::En);
    let mut s = Settings::default();
    s.lang = Lang::En;

    // Đi qua đúng đường serde mà phần mềm dùng để ghi ra settings.json
    let json = serde_json::to_string(&s).expect("ghi được");
    assert!(json.contains("\"lang\":\"en\""), "phải ghi mã ngôn ngữ vào file: {}", &json[..120.min(json.len())]);

    let back: Settings = serde_json::from_str(&json).expect("đọc lại được");
    assert_eq!(back.lang, Lang::En);
    assert!(back.groups.iter().any(|g| g.name == "01-Images"));

    // File cũ chưa có khoá lang thì phải mặc định tiếng Việt, không được lỗi
    let old = json.replace("\"lang\":\"en\",", "");
    let legacy: Settings = serde_json::from_str(&old).expect("file cũ vẫn đọc được");
    assert_eq!(legacy.lang, Lang::Vi, "file cũ không có khoá lang phải về tiếng Việt");

    foldu_lib::i18n::set_lang(Lang::Vi);
}

/// Hộp chọn ngôn ngữ chỉ được hiện đúng một lần, ở lần mở đầu tiên
#[test]
fn chi_hoi_ngon_ngu_o_lan_mo_dau_tien() {
    use foldu_lib::i18n::Lang;
    let _g = foldu_lib::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Cài đặt mới tinh: chưa hỏi
    let fresh = Settings::default();
    assert!(!fresh.lang_picked, "lần đầu phải hỏi ngôn ngữ");

    // Sau khi người dùng chọn thì cờ được ghi vào file và đọc lại vẫn còn
    let mut picked = Settings::default();
    picked.lang_picked = true;
    let json = serde_json::to_string(&picked).unwrap();
    let back: Settings = serde_json::from_str(&json).unwrap();
    assert!(back.lang_picked, "đã chọn rồi thì lần sau không hỏi lại");

    // File settings.json cũ từ bản trước chưa có khoá này: coi như chưa hỏi.
    // Người đang dùng bản cũ sẽ được hỏi một lần rồi thôi, chứ không bị bỏ qua.
    let legacy = json.replace(",\"langPicked\":true", "");
    assert!(!legacy.contains("langPicked"));
    let old: Settings = serde_json::from_str(&legacy).expect("file cũ vẫn đọc được");
    assert!(!old.lang_picked);

    // Đoán theo hệ điều hành phải ra một trong hai, không được hoảng
    let sys = foldu_lib::i18n::system_lang();
    assert!(matches!(sys, Lang::Vi | Lang::En));
    eprintln!("\n  Ngôn ngữ hệ điều hành đoán được: {:?}\n", sys);
}

// ═══════════════════════════════ 12. Đổi tên hàng loạt ═══════════════════════

/// Dựng một RenameSpec từ mấy mảnh cho gọn
fn rename_spec(parts: Vec<foldu_lib::rename::Part>) -> foldu_lib::rename::RenameSpec {
    foldu_lib::rename::RenameSpec {
        enabled: true,
        in_place: true,
        parts,
        transforms: foldu_lib::rename::Transforms::default(),
    }
}
fn rp(kind: &str) -> foldu_lib::rename::Part {
    foldu_lib::rename::Part { kind: kind.into(), text: String::new(), format: String::new(), width: 0, start: 0 }
}
fn rtext(s: &str) -> foldu_lib::rename::Part {
    let mut p = rp("text");
    p.text = s.into();
    p
}

#[test]
fn doi_ten_hang_loat_de_yen_cho_va_dat_so_thu_tu() {
    let fx = Fixture::new("rename");
    // Ba ảnh trong cùng thư mục, tên vô nghĩa
    let originals = vec![
        fx.file("anh/IMG_0003.jpg", "c"),
        fx.file("anh/IMG_0001.jpg", "a"),
        fx.file("anh/IMG_0002.jpg", "b"),
    ];

    let mut prof = profile(&[]);
    prof.recursive = true;
    // Mẫu: "KyNghi_" + số 3 chữ số bắt đầu từ 1
    let mut counter = rp("counter");
    counter.width = 3;
    counter.start = 1;
    prof.rename = rename_spec(vec![rtext("KyNghi_"), counter]);

    let (_, plan) = run_plan(&fx.root, &prof);

    // Mọi thao tác phải nằm trong thư mục "anh", KHÔNG bị chuyển đi đâu
    for o in &plan.ops {
        if o.action == OpAction::Move {
            assert_eq!(
                o.dest.parent().unwrap(),
                fx.root.join("anh"),
                "đổi tên tại chỗ không được chuyển file ra thư mục khác: {:?}",
                o.dest
            );
        }
    }

    let res = apply(&plan);
    assert_eq!(res.failed, 0);

    // Tên mới đánh số theo thứ tự tên gốc: IMG_0001 -> 001, ...
    let after: Vec<String> = walk(&fx.root.join("anh"))
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(after.contains(&"KyNghi_001.jpg".to_string()), "thiếu 001: {:?}", after);
    assert!(after.contains(&"KyNghi_002.jpg".to_string()), "thiếu 002: {:?}", after);
    assert!(after.contains(&"KyNghi_003.jpg".to_string()), "thiếu 003: {:?}", after);
    // Không còn tên gốc nào
    assert!(!after.iter().any(|n| n.starts_with("IMG_")), "còn tên gốc: {:?}", after);

    // ── KHÔI PHỤC: bấm một nút phải về ĐÚNG tên gốc, từng cái một
    let undo = executor::undo_session(&res.session, None, |_, _| {}).unwrap();
    assert!(undo.conflicts.is_empty(), "hoàn tác không được xung đột: {:?}", undo.conflicts);
    assert_eq!(undo.restored, res.done, "phải trả về đủ số file");
    for p in &originals {
        assert!(p.exists(), "tên gốc phải trở lại đúng như cũ: {:?}", p);
    }
    let after_undo: Vec<String> = walk(&fx.root.join("anh"))
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(!after_undo.iter().any(|n| n.starts_with("KyNghi")), "không được sót tên mới: {:?}", after_undo);
}

#[test]
fn doi_ten_khong_bao_gio_doi_duoi_file() {
    let fx = Fixture::new("rename-ext");
    fx.file("tai lieu.pdf", "x");
    let mut prof = profile(&[]);
    // Người dùng cố nhét đuôi khác vào mẫu
    prof.rename = rename_spec(vec![rtext("hopdong.exe")]);
    let (_, plan) = run_plan(&fx.root, &prof);
    let op = plan.ops.iter().find(|o| o.action == OpAction::Move).expect("phải có thao tác");
    assert!(op.final_name.ends_with(".pdf"), "đuôi gốc .pdf phải được giữ: {}", op.final_name);
    assert!(!op.final_name.to_lowercase().ends_with(".exe"), "không được đổi thành .exe: {}", op.final_name);
}

#[test]
fn hai_file_ra_cung_ten_thi_them_so_khong_de_len_nhau() {
    let fx = Fixture::new("rename-clash");
    let a = fx.file("a.txt", "NOI DUNG A");
    let b = fx.file("b.txt", "NOI DUNG B");
    let mut prof = profile(&[]);
    // Mẫu cố định -> cả hai muốn thành "tailieu.txt"
    prof.rename = rename_spec(vec![rtext("tailieu")]);
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);
    assert_eq!(res.failed, 0);

    let names: Vec<String> = walk(&fx.root).iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    // Một cái là "tailieu.txt", cái kia được thêm số
    assert!(names.contains(&"tailieu.txt".to_string()), "{:?}", names);
    assert!(names.iter().any(|n| n != "tailieu.txt" && n.starts_with("tailieu")), "phải có bản thêm số: {:?}", names);
    // Cả hai nội dung còn nguyên, không mất file nào
    let mut contents: Vec<String> = walk(&fx.root).iter().map(|p| fs::read_to_string(p).unwrap()).collect();
    contents.sort();
    let _ = (a, b);
    assert_eq!(contents, vec!["NOI DUNG A", "NOI DUNG B"], "không được mất nội dung nào: {:?}", contents);
}

#[test]
fn doi_ten_kem_bo_dau_va_chu_thuong() {
    let fx = Fixture::new("rename-fold");
    fx.file("Hợp Đồng Thuê Nhà.docx", "x");
    let mut prof = profile(&[]);
    prof.rename = foldu_lib::rename::RenameSpec {
        enabled: true,
        in_place: true,
        parts: vec![rp("name")],
        transforms: foldu_lib::rename::Transforms { strip_diacritics: true, lowercase: true, kebab: true },
    };
    let (_, plan) = run_plan(&fx.root, &prof);
    let op = plan.ops.iter().find(|o| o.renamed).expect("phải có đổi tên");
    assert_eq!(op.final_name, "hop-dong-thue-nha.docx", "{}", op.final_name);
}

// ═══════════════════ 13. Dọn cả thư mục con: vỏ rỗng phải được dọn theo

#[test]
fn don_ca_thu_muc_con_thi_vo_rong_con_lai_phai_duoc_don_sach() {
    let fx = Fixture::new("sweep");
    fx.file("tai-lieu/2023/quy-1/bao cao.pdf", "a");
    fx.file("tai-lieu/2023/quy-2/hop dong.docx", "b");
    fx.file("anh/ky-nghi/bien.jpg", "c");
    let goc_cua_nguoi_dung = fx.root.clone();

    let mut prof = profile(&["TYPE"]);
    prof.recursive = true;
    prof.safety.clean_empty_dirs = true;

    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);
    assert_eq!(res.failed, 0);

    // Mọi vỏ thư mục rỗng phải biến mất
    for rong in ["tai-lieu/2023/quy-1", "tai-lieu/2023/quy-2", "tai-lieu/2023", "tai-lieu", "anh/ky-nghi", "anh"] {
        assert!(
            !fx.root.join(rong).exists(),
            "thư mục rỗng '{}' đáng lẽ phải được dọn đi",
            rong
        );
    }
    assert!(res.removed_dirs >= 6, "phải dọn ít nhất 6 vỏ rỗng, thực tế {}", res.removed_dirs);
    // Nhưng thư mục người dùng chọn thì không bao giờ được đụng vào
    assert!(goc_cua_nguoi_dung.exists(), "thư mục gốc người dùng chọn phải còn nguyên");

    // Hoàn tác phải dựng lại đúng cấu trúc cũ
    let undo = executor::undo_session(&res.session, None, |_, _| {}).unwrap();
    assert!(undo.conflicts.is_empty(), "không được có xung đột: {:?}", undo.conflicts);
    for p in ["tai-lieu/2023/quy-1/bao cao.pdf", "tai-lieu/2023/quy-2/hop dong.docx", "anh/ky-nghi/bien.jpg"] {
        assert!(fx.root.join(p).exists(), "file phải trở về đúng chỗ cũ: {}", p);
    }
}

#[test]
fn tat_don_vo_rong_thi_thu_muc_cu_van_con_nguyen() {
    let fx = Fixture::new("nosweep");
    fx.file("tai-lieu/bao cao.pdf", "a");

    let mut prof = profile(&["TYPE"]);
    prof.safety.clean_empty_dirs = false;
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);

    assert_eq!(res.removed_dirs, 0);
    assert!(fx.root.join("tai-lieu").exists(), "tắt tuỳ chọn thì vỏ rỗng phải còn lại");
}

#[test]
fn che_do_sao_chep_khong_bao_gio_don_thu_muc_cu() {
    let fx = Fixture::new("copynosweep");
    fx.file("tai-lieu/bao cao.pdf", "a");

    let mut prof = profile(&["TYPE"]);
    prof.mode = Mode::Copy;
    prof.safety.clean_empty_dirs = true;
    let (_, plan) = run_plan(&fx.root, &prof);
    let res = apply(&plan);

    assert_eq!(res.removed_dirs, 0, "chép ra bản mới thì chỗ cũ vẫn còn file, không được dọn");
    assert!(fx.root.join("tai-lieu/bao cao.pdf").exists());
}

// ═══════════════════════════════════ 12. Sắp xếp nguyên ổ đĩa / phân vùng

#[test]
fn goc_o_he_thong_bi_chan_con_phan_vung_du_lieu_thi_duoc_phep() {
    let sys = foldu_lib::safety::system_drive();
    let sys_root = PathBuf::from(format!("{}:\\", sys));

    let c = foldu_lib::safety::check_source(&sys_root);
    assert!(!c.ok, "gốc ổ chứa Windows phải bị chặn");
    assert_eq!(c.level, "block");
    assert!(c.reason.contains("Windows"));

    // Tim mot phan vung du lieu that de kiem tra chieu nguoc lai
    let data = foldu_lib::safety::list_drives()
        .into_iter()
        .find(|d| d.kind == "fixed" && !d.is_system);
    if let Some(d) = data {
        let chk = foldu_lib::safety::check_source(&PathBuf::from(&d.path));
        assert!(chk.ok, "phân vùng dữ liệu phải được phép: {}", chk.reason);
        assert_eq!(chk.level, "warn", "và phải kèm cảnh báo");
        assert!(foldu_lib::safety::is_drive_root(&PathBuf::from(&d.path)));
    } else {
        eprintln!("bỏ qua nửa sau: máy này không có phân vùng dữ liệu riêng");
    }
}

#[test]
fn list_drives_tra_ve_dung_dung_luong_va_khoa_o_he_thong() {
    let drives = foldu_lib::safety::list_drives();
    assert!(!drives.is_empty(), "phải thấy ít nhất một ổ đĩa");
    for d in &drives {
        assert!(d.total > 0);
        assert!(d.used + d.free <= d.total + 1024 * 1024, "used + free phải khớp total");
        if d.is_system {
            assert!(!d.selectable, "ổ hệ thống không được cho chọn");
        }
    }
    assert!(drives.iter().any(|d| d.is_system), "phải nhận ra ổ hệ thống");
}

/// Quet THAT nguyen mot phan vung du lieu tren may nay.
/// Muc tieu: chung minh cac lop bao ve khong cho file he thong lot vao ke hoach.
#[test]
fn quet_nguyen_phan_vung_khong_bao_gio_dung_vao_muc_he_thong() {
    let Some(drive) = foldu_lib::safety::list_drives()
        .into_iter()
        .find(|d| d.kind == "fixed" && !d.is_system)
    else {
        eprintln!("máy này không có phân vùng dữ liệu riêng — bỏ qua");
        return;
    };

    let root = PathBuf::from(&drive.path);
    let prof = profile(&["TYPE"]);
    let t0 = std::time::Instant::now();
    let scan = Scanner::new(&prof, vec![]).run(&[root.clone()], |_, _| {});
    let elapsed = t0.elapsed();

    eprintln!(
        "\n  Ổ {}: {} file · {} · {} thư mục · {:?}",
        drive.letter,
        scan.files.len(),
        foldu_lib::util::format_bytes(scan.stats.total_bytes),
        scan.stats.scanned_dirs,
        elapsed
    );
    eprintln!(
        "  Bảo vệ: {} mục hệ thống · {} thư mục ứng dụng · {} thư mục dự án · {} file đám mây",
        scan.stats.system_protected,
        scan.stats.app_folders_protected,
        scan.stats.project_folders,
        scan.stats.cloud_skipped
    );

    assert!(scan.stats.whole_drive, "phải nhận ra đang quét nguyên ổ");

    // Khong mot file nao trong ke hoach duoc nam trong vung cam
    const FORBIDDEN: &[&str] = &[
        "$recycle.bin",
        "system volume information",
        "config.msi",
        "pagefile.sys",
        "hiberfil.sys",
        "swapfile.sys",
        "dumpstack.log",
        "$winreagent",
        "\\windows\\",
        "\\program files",
        "\\programdata\\",
    ];
    for f in &scan.files {
        let low = f.path.to_string_lossy().to_lowercase();
        for bad in FORBIDDEN {
            assert!(
                !low.contains(bad),
                "lọt vào vùng cấm '{}': {}",
                bad,
                f.path.display()
            );
        }
    }

    // Va ke hoach sinh ra cung phai sach
    let settings = Settings::default();
    let plan = planner::build_plan(&scan.files, &prof, &settings, &[root.clone()], |_, _, _| {});
    for o in &plan.ops {
        let low = o.dest.to_string_lossy().to_lowercase();
        for bad in FORBIDDEN {
            assert!(!low.contains(bad), "đích rơi vào vùng cấm: {}", o.dest.display());
        }
    }
    eprintln!("  Kế hoạch: {} thao tác, {} thư mục mới\n", plan.summary.total, plan.summary.new_folders);
}

// ══════════════════════════════════════════════════════════════════ Tiện ích

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}
