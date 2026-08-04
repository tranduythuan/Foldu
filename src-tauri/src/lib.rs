//! Foldu — lop lenh Tauri noi giao dien voi lop loi.

pub mod analytics;
pub mod clustering;
pub mod config;
pub mod criteria;
pub mod dedup;
pub mod executor;
pub mod geo;
pub mod i18n;
pub mod journal;
pub mod media;
pub mod phash;
pub mod planner;
pub mod rename;
pub mod safety;
pub mod scanner;
pub mod util;

use analytics::Analytics;
use config::{Preset, Profile, Settings};
use executor::{ExecResult, UndoResult};
use journal::SessionInfo;
use once_cell::sync::Lazy;
use planner::{Plan, PlanOp};
use safety::{PathCheck, PreflightOp, PreflightResult};
use scanner::{ScanResult, Scanner};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------- Trang thai

/// Ket qua phan nang (trung lap + anh gan giong) da tinh, giu lai de khoi tinh
/// lai moi lan doi cach sap xep. `key` la cac cai dat trung lap co anh huong toi
/// ket qua; doi scan hoac doi mot trong so do thi cache bi bo.
struct DupNearCache {
    key: (bool, config::DupStrategy, bool, u32),
    dup: dedup::DupReport,
    near: phash::NearReport,
}

#[derive(Default)]
struct AppState {
    scan: Option<ScanResult>,
    plan: Option<Plan>,
    roots: Vec<PathBuf>,
    settings: Option<Settings>,
    dup_near_cache: Option<DupNearCache>,
}

static STATE: Lazy<Mutex<AppState>> = Lazy::new(|| Mutex::new(AppState::default()));
static CANCEL: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

fn settings() -> Settings {
    let mut s = STATE.lock().unwrap();
    if s.settings.is_none() {
        s.settings = Some(config::load_settings());
    }
    s.settings.clone().unwrap()
}

// ----------------------------------------------------------------- Su kien

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    phase: String,
    note: String,
    current: usize,
    total: usize,
}

fn emit(app: &AppHandle, phase: &str, note: &str, current: usize, total: usize) {
    let _ = app.emit(
        "foldu:progress",
        Progress {
            phase: phase.into(),
            note: note.into(),
            current,
            total,
        },
    );
}

// ------------------------------------------------------------------- Lenh

#[tauri::command]
async fn pick_folders() -> Vec<String> {
    match rfd::AsyncFileDialog::new()
        .set_title(i18n::t("ui.pickFolder"))
        .pick_folders()
        .await
    {
        Some(list) => list
            .into_iter()
            .map(|h| h.path().to_string_lossy().to_string())
            .collect(),
        None => vec![],
    }
}

#[tauri::command]
async fn pick_destination() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title(i18n::t("ui.pickDest"))
        .pick_folder()
        .await
        .map(|h| h.path().to_string_lossy().to_string())
}

#[tauri::command]
fn check_path(path: String) -> PathCheck {
    safety::check_source(&PathBuf::from(path))
}

/// Liet ke moi o dia dang gan, kem dung luong — dung cho man chon o dia
#[tauri::command]
fn list_drives() -> Vec<safety::DriveInfo> {
    safety::list_drives()
}

#[tauri::command]
fn check_destination(source: String, dest: String) -> PathCheck {
    safety::check_destination(&PathBuf::from(source), &PathBuf::from(dest))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanOutcome {
    stats: scanner::ScanStats,
    analytics: Analytics,
    skipped: Vec<scanner::SkippedItem>,
    blocked: Vec<String>,
}

#[tauri::command]
async fn scan_folders(
    app: AppHandle,
    paths: Vec<String>,
    profile: Profile,
) -> Result<ScanOutcome, String> {
    let st = settings();
    tauri::async_runtime::spawn_blocking(move || {
        let mut roots: Vec<PathBuf> = Vec::new();
        let mut blocked: Vec<String> = Vec::new();
        for p in &paths {
            let pb = PathBuf::from(p);
            let c = safety::check_source(&pb);
            if c.ok {
                roots.push(pb);
            } else {
                blocked.push(format!("{}: {}", p, c.reason));
            }
        }
        if roots.is_empty() {
            return Err(if blocked.is_empty() {
                i18n::t("msg.noFolderPicked").to_string()
            } else {
                blocked.join("\n")
            });
        }

        // Loai tru thu muc dich khoi pham vi quet de tranh de quy
        let exclude: Vec<PathBuf> = profile
            .destination
            .as_ref()
            .filter(|d| !d.is_empty())
            .map(|d| vec![PathBuf::from(d)])
            .unwrap_or_default();

        emit(&app, "scan", i18n::t("prog.scanning"), 0, 0);
        let scanner = Scanner::new(&profile, exclude);
        let a = app.clone();
        let result = scanner.run(&roots, |n, p| {
            emit(
                &a,
                "scan",
                &i18n::tf("prog.scanningAt", &[&short_path(&p.to_string_lossy())]),
                n,
                0,
            );
        });

        emit(&app, "analyze", i18n::t("prog.analyzing"), 0, 0);
        let an = analytics::analyze(&result, &st, &roots);
        let outcome = ScanOutcome {
            stats: result.stats.clone(),
            analytics: an,
            skipped: result.skipped.iter().take(300).cloned().collect(),
            blocked,
        };

        let mut s = STATE.lock().unwrap();
        s.scan = Some(result);
        s.roots = roots;
        s.plan = None;
        s.dup_near_cache = None; // tap file doi -> ket qua trung lap/anh cu khong con dung
        emit(&app, "done", i18n::t("prog.scanDone"), 1, 1);
        Ok(outcome)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanOutcome {
    summary: planner::PlanSummary,
    warnings: Vec<String>,
    folders: Vec<String>,
    dup_report: dedup::DupReport,
    near_report: phash::NearReport,
    elapsed_ms: u64,
    /// Trang dau tien cua danh sach thao tac; phan con lai lay qua `plan_page`
    first_page: Vec<PlanOp>,
}

#[tauri::command]
async fn make_plan(app: AppHandle, profile: Profile) -> Result<PlanOutcome, String> {
    let st = settings();
    tauri::async_runtime::spawn_blocking(move || {
        let (files, roots) = {
            let s = STATE.lock().unwrap();
            match &s.scan {
                Some(sc) => (sc.files.clone(), s.roots.clone()),
                None => return Err(i18n::t("msg.noScan").to_string()),
            }
        };

        // Phan nang (trung lap + anh gan giong) chi phu thuoc tap file va cai dat
        // trung lap. Cache lai theo key duoi day; doi tieu chi sap xep thi tai su
        // dung, khong bam lai -> preview cap nhat tuc thi.
        let key = (
            profile.duplicates.enabled,
            profile.duplicates.strategy,
            profile.duplicates.near_images,
            profile.duplicates.near_threshold,
        );
        let cached = {
            let s = STATE.lock().unwrap();
            s.dup_near_cache
                .as_ref()
                .filter(|c| c.key == key)
                .map(|c| (c.dup.clone(), c.near.clone()))
        };
        let (dup_report, near_report) = match cached {
            Some(rep) => rep,
            None => {
                let a = app.clone();
                let rep = planner::compute_dup_near(&files, &profile, |note, cur, total| {
                    emit(&a, "plan", note, cur, total);
                });
                STATE.lock().unwrap().dup_near_cache = Some(DupNearCache {
                    key,
                    dup: rep.0.clone(),
                    near: rep.1.clone(),
                });
                rep
            }
        };

        let a = app.clone();
        let plan = planner::build_plan_with_reports(
            &files,
            &profile,
            &st,
            &roots,
            |note, cur, total| emit(&a, "plan", note, cur, total),
            dup_report,
            near_report,
        );

        let outcome = PlanOutcome {
            summary: plan.summary.clone(),
            warnings: plan.warnings.clone(),
            folders: plan.folders.iter().take(4000).cloned().collect(),
            dup_report: dedup::DupReport {
                groups: plan.dup_report.groups.iter().take(200).cloned().collect(),
                ..plan.dup_report.clone()
            },
            near_report: phash::NearReport {
                groups: plan.near_report.groups.iter().take(200).cloned().collect(),
                ..plan.near_report.clone()
            },
            elapsed_ms: plan.elapsed_ms,
            first_page: plan.ops.iter().take(300).cloned().collect(),
        };

        STATE.lock().unwrap().plan = Some(plan);
        emit(&app, "done", i18n::t("prog.planDone"), 1, 1);
        Ok(outcome)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanPage {
    ops: Vec<PlanOp>,
    total: usize,
}

/// Loc danh sach thao tac theo dung mot bo quy tac, dung chung cho `plan_page` va
/// `plan_ids` — hai lenh phai nhin thay y het mot tap thi nut "bo tick ca nhom" moi
/// khop voi cai nguoi dung dang xem.
/// `filter`: "" | "moved" | "renamed" | "dup" | "keep"
fn filter_ops<'a>(plan: &'a Plan, filter: &str, search: &str) -> Vec<&'a PlanOp> {
    let needle = util::norm_key(search);
    plan.ops
        .iter()
        .filter(|o| match filter {
            "moved" => matches!(
                o.action,
                planner::OpAction::Move | planner::OpAction::Copy | planner::OpAction::Hardlink
            ),
            "renamed" => o.renamed,
            "dup" => matches!(
                o.action,
                planner::OpAction::Quarantine | planner::OpAction::Recycle
            ),
            "keep" => o.action == planner::OpAction::Keep,
            _ => true,
        })
        .filter(|o| {
            needle.is_empty()
                || util::norm_key(&o.src.to_string_lossy()).contains(&needle)
                || util::norm_key(&o.dest.to_string_lossy()).contains(&needle)
        })
        .collect()
}

/// Lay tung trang danh sach thao tac — tranh nem 100.000 dong sang giao dien mot luc.
#[tauri::command]
fn plan_page(offset: usize, limit: usize, filter: String, search: String) -> PlanPage {
    let s = STATE.lock().unwrap();
    let plan = match &s.plan {
        Some(p) => p,
        None => return PlanPage { ops: vec![], total: 0 },
    };
    let filtered = filter_ops(plan, &filter, &search);

    PlanPage {
        total: filtered.len(),
        ops: filtered
            .into_iter()
            .skip(offset)
            .take(limit.clamp(1, 2000))
            .cloned()
            .collect(),
    }
}

/// Ma so cua MOI thao tac khop bo loc, de giao dien bo tick ca nhom trong mot lan.
/// Chi doc, khong dong toi ke hoach. Bo qua thao tac Keep vi chung von khong lam gi.
#[tauri::command]
fn plan_ids(filter: String, search: String) -> Vec<u32> {
    let s = STATE.lock().unwrap();
    let plan = match &s.plan {
        Some(p) => p,
        None => return vec![],
    };
    filter_ops(plan, &filter, &search)
        .into_iter()
        .filter(|o| o.action != planner::OpAction::Keep)
        .map(|o| o.id)
        .collect()
}

#[tauri::command]
async fn run_preflight(deselected: Vec<u32>) -> Result<PreflightResult, String> {
    // Preflight mo tung file de do khoa + tao file thu ghi -> nang ve I/O (nhat la
    // voi .exe/.dll bi antivirus quet). Phai chay NGOAI luong UI, neu khong cua so
    // se dong bang "Not Responding". Cac lenh nang khac cung theo mau spawn_blocking nay.
    tauri::async_runtime::spawn_blocking(move || {
        let (ops, copies, roots) = {
            let s = STATE.lock().unwrap();
            let plan = s.plan.as_ref().ok_or_else(|| i18n::t("msg.noPlan").to_string())?;
            let de: HashSet<u32> = deselected.into_iter().collect();

            let ops: Vec<PreflightOp> = plan
                .ops
                .iter()
                .filter(|o| {
                    o.selected
                        && !de.contains(&o.id)
                        && !matches!(o.action, planner::OpAction::Keep | planner::OpAction::Skip)
                })
                .map(|o| PreflightOp {
                    id: o.id,
                    src: o.src.clone(),
                    dest: o.dest.clone(),
                    size: o.size,
                })
                .collect();

            let copies = matches!(plan.mode, config::Mode::Copy);
            (ops, copies, plan.roots.clone())
        };
        Ok(safety::preflight(&ops, copies, &roots))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn apply_plan(
    app: AppHandle,
    profile_name: String,
    deselected: Vec<u32>,
    skip_ids: Vec<u32>,
) -> Result<ExecResult, String> {
    CANCEL.store(false, Ordering::Relaxed);
    let cancel = CANCEL.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut plan = {
            let s = STATE.lock().unwrap();
            s.plan.clone().ok_or_else(|| i18n::t("msg.noPlan").to_string())?
        };
        let de: HashSet<u32> = deselected.into_iter().collect();
        for o in plan.ops.iter_mut() {
            if de.contains(&o.id) {
                o.selected = false;
            }
        }
        let skips: HashSet<u32> = skip_ids.into_iter().collect();

        let a = app.clone();
        let res = executor::execute(&plan, &profile_name, &skips, cancel, |p| {
            emit(
                &a,
                "apply",
                &i18n::tf("prog.applying", &[&short_path(&p.current)]),
                p.index,
                p.total,
            );
        })?;
        emit(&app, "done", i18n::t("prog.done"), 1, 1);
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_run() {
    CANCEL.store(true, Ordering::Relaxed);
}

// -------------------------------------------------------------------- Lich su

#[tauri::command]
fn history() -> Vec<SessionInfo> {
    journal::list_sessions()
}

#[tauri::command]
fn interrupted() -> Vec<SessionInfo> {
    journal::interrupted_sessions()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDetail {
    info: SessionInfo,
    moves: Vec<SessionMove>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionMove {
    seq: usize,
    src: String,
    dest: String,
    action: String,
    certain: bool,
}

#[tauri::command]
fn session_detail(session: String) -> Result<SessionDetail, String> {
    let d = journal::read_session(&session).ok_or_else(|| i18n::t("msg.noSessionFound").to_string())?;
    let mut moves: Vec<SessionMove> = d
        .completed
        .iter()
        .map(|(seq, s, t, a)| SessionMove {
            seq: *seq,
            src: s.to_string_lossy().to_string(),
            dest: t.to_string_lossy().to_string(),
            action: a.clone(),
            certain: true,
        })
        .collect();
    moves.extend(d.uncertain.iter().map(|(seq, s, t, a)| SessionMove {
        seq: *seq,
        src: s.to_string_lossy().to_string(),
        dest: t.to_string_lossy().to_string(),
        action: a.clone(),
        certain: false,
    }));
    moves.sort_by_key(|m| m.seq);
    Ok(SessionDetail { info: d.info, moves })
}

#[tauri::command]
async fn undo(
    app: AppHandle,
    session: String,
    seqs: Option<Vec<usize>>,
) -> Result<UndoResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sel = seqs.map(|v| v.into_iter().collect::<HashSet<usize>>());
        let a = app.clone();
        let r = executor::undo_session(&session, sel, |cur, total| {
            emit(&a, "undo", i18n::t("prog.undoing"), cur, total);
        })?;
        emit(&app, "done", i18n::t("prog.undone"), 1, 1);
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ------------------------------------------------------------------- Cau hinh

#[tauri::command]
fn get_settings() -> Settings {
    settings()
}

#[tauri::command]
fn set_settings(s: Settings) -> Result<(), String> {
    config::save_settings(&s)?;
    STATE.lock().unwrap().settings = Some(s);
    Ok(())
}

/// Đổi ngôn ngữ. Trả về bộ cài đặt mới để giao diện vẽ lại ngay.
///
/// Nếu bảng nhóm file và bảng từ khoá vẫn đúng y bộ mặc định của ngôn ngữ cũ
/// (tức người dùng chưa sửa gì) thì đổi luôn sang bộ mặc định của ngôn ngữ mới.
/// Ai đã sửa tay thì giữ nguyên, vì đó là dữ liệu của họ.
#[tauri::command]
fn set_lang(lang: String) -> Settings {
    let new = i18n::Lang::from_code(&lang);
    let old = i18n::lang();
    let mut s = settings();
    if new == old {
        return s;
    }

    let was_default = config::matches_defaults_of(&s, old);
    i18n::set_lang(new);
    if was_default {
        s.groups = config::default_groups();
        s.keywords = config::default_keywords();
    }
    s.lang = new;
    s.lang_picked = true;
    let _ = config::save_settings(&s);
    STATE.lock().unwrap().settings = Some(s.clone());
    s
}

/// Người dùng đã trả lời hộp chọn ngôn ngữ ở lần mở đầu tiên, kể cả khi giữ
/// nguyên ngôn ngữ được đoán sẵn. Lần sau không hỏi lại nữa.
#[tauri::command]
fn confirm_lang() -> Settings {
    let mut s = settings();
    s.lang_picked = true;
    s.lang = i18n::lang();
    let _ = config::save_settings(&s);
    STATE.lock().unwrap().settings = Some(s.clone());
    s
}

#[tauri::command]
fn get_presets() -> Vec<Preset> {
    config::presets()
}

#[tauri::command]
fn get_catalog() -> serde_json::Value {
    criteria::catalog()
}

#[tauri::command]
fn default_profile() -> Profile {
    Profile::default()
}

// -------------------------------------------------------------------- Tien ich

#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    #[cfg(windows)]
    {
        let arg = if p.is_dir() {
            p.to_string_lossy().to_string()
        } else {
            format!("/select,{}", p.to_string_lossy())
        };
        std::process::Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = p;
    }
    Ok(())
}

#[tauri::command]
fn export_report(format: String) -> Result<String, String> {
    let s = STATE.lock().unwrap();
    let plan = s.plan.as_ref().ok_or_else(|| i18n::t("msg.noReport").to_string())?;
    config::ensure_dirs();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();

    let (file, body) = if format == "csv" {
        let mut out = format!(
            "{};{};{};{};{}\n",
            i18n::t("ui.colAction"),
            i18n::t("ui.colFrom"),
            i18n::t("ui.colTo"),
            i18n::t("ui.colSize"),
            i18n::t("ui.colReason")
        );
        for o in &plan.ops {
            out.push_str(&format!(
                "{:?};{};{};{};{}\n",
                o.action,
                csv_cell(&o.src.to_string_lossy()),
                csv_cell(&o.dest.to_string_lossy()),
                o.size,
                csv_cell(&o.reason)
            ));
        }
        (config::reports_dir().join(format!("bao-cao-{}.csv", stamp)), out)
    } else {
        (
            config::reports_dir().join(format!("bao-cao-{}.html", stamp)),
            render_html_report(plan),
        )
    };

    // BOM UTF-8 de Excel mo tieng Viet khong bi loi font
    let mut data = Vec::from([0xEF, 0xBB, 0xBF]);
    data.extend_from_slice(body.as_bytes());
    std::fs::write(&file, data).map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().to_string())
}

fn csv_cell(s: &str) -> String {
    s.replace(';', ",").replace(['\n', '\r'], " ")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_html_report(plan: &Plan) -> String {
    let s = &plan.summary;
    let mut rows = String::new();
    for o in plan.ops.iter().take(20000) {
        rows.push_str(&format!(
            "<tr><td>{:?}</td><td>{}</td><td>{}</td><td class=n>{}</td><td>{}</td></tr>",
            o.action,
            html_escape(&o.src.to_string_lossy()),
            html_escape(&o.dest.to_string_lossy()),
            util::format_bytes(o.size),
            html_escape(&o.reason)
        ));
    }
    format!(
        r#"<meta charset="utf-8"><title>Báo cáo sắp xếp</title>
<style>
body{{font:14px/1.6 Segoe UI,system-ui,sans-serif;background:#0b0b0f;color:#e5e5ea;margin:0;padding:32px}}
h1{{font-size:22px;margin:0 0 4px}} .sub{{color:#8b8b96;margin-bottom:24px}}
.cards{{display:flex;gap:12px;flex-wrap:wrap;margin-bottom:24px}}
.card{{background:#16161c;border:1px solid #26262f;border-radius:10px;padding:14px 18px;min-width:150px}}
.card b{{display:block;font-size:22px;color:#a5b4fc}}
table{{width:100%;border-collapse:collapse;font-size:12.5px}}
th,td{{text-align:left;padding:7px 10px;border-bottom:1px solid #22222b;vertical-align:top;word-break:break-all}}
th{{color:#8b8b96;font-weight:600;position:sticky;top:0;background:#0b0b0f}}
td.n{{white-space:nowrap;color:#8b8b96}}
</style>
<h1>{}</h1>
<div class=sub>{} {} · {} {:?}</div>
<div class=cards>
<div class=card><b>{}</b>{}</div>
<div class=card><b>{}</b>{}</div>
<div class=card><b>{}</b>{}</div>
<div class=card><b>{}</b>{}</div>
<div class=card><b>{}</b>{}</div>
<div class=card><b>{}</b>{}</div>
</div>
<table><thead><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr></thead>
<tbody>{}</tbody></table>"#,
        i18n::t("ui.reportTitle"),
        i18n::t("ui.reportMade"),
        chrono::Local::now().format("%d/%m/%Y %H:%M"),
        i18n::t("ui.reportMode"),
        plan.mode,
        s.total, i18n::t("ui.sumTotal"),
        s.moves, i18n::t("ui.sumMoves"),
        s.renames, i18n::t("ui.sumRenames"),
        s.duplicates, i18n::t("ui.sumDupes"),
        s.new_folders, i18n::t("ui.sumFolders"),
        util::format_bytes(s.dup_wasted), i18n::t("ui.sumWasted"),
        i18n::t("ui.colAction"), i18n::t("ui.colFrom"), i18n::t("ui.colTo"), i18n::t("ui.colSize"), i18n::t("ui.colReason"),
        rows
    )
}

fn short_path(p: &str) -> String {
    let n = p.chars().count();
    if n <= 64 {
        return p.to_string();
    }
    let tail: String = p.chars().skip(n - 58).collect();
    format!("...{}", tail)
}

// --------------------------------------------------------------------- Khoi dong

/// Thu nhỏ cửa sổ cho vừa màn hình thật của người dùng.
///
/// Kích thước mặc định trong tauri.conf.json là kích thước mong muốn, không phải
/// kích thước bảo đảm vừa. Trên máy dùng tỷ lệ hiển thị 125% hoặc 150%, màn hình
/// 1920x1080 chỉ còn 1536x864 hoặc 1280x720 theo đơn vị logic — cửa sổ to hơn thế
/// sẽ thò xuống dưới thanh tác vụ và che mất chân thanh bên.
fn fit_window_to_screen(app: &tauri::App) {
    use tauri::{LogicalSize, Manager};

    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(mon)) = win.current_monitor() else {
        return;
    };

    let scale = mon.scale_factor();
    let screen: LogicalSize<f64> = mon.size().to_logical(scale);

    let (Ok(inner), Ok(outer)) = (win.inner_size(), win.outer_size()) else {
        return;
    };
    let inner: LogicalSize<f64> = inner.to_logical(scale);
    let outer: LogicalSize<f64> = outer.to_logical(scale);

    // `set_size` đặt vùng NỘI DUNG, nhưng thứ phải vừa màn hình là cả KHUNG NGOÀI
    // gồm thanh tiêu đề và viền. Bỏ qua phần chênh này là chỗ dễ tính thiếu nhất:
    // cửa sổ trông như vừa đủ rồi vẫn thò xuống dưới thanh tác vụ đúng bằng chiều
    // cao thanh tiêu đề.
    let chrome_w = (outer.width - inner.width).max(0.0);
    let chrome_h = (outer.height - inner.height).max(0.0);

    // Chừa chỗ cho thanh tác vụ. Không có API lấy vùng làm việc đa nền tảng nên
    // trừ ước lượng rộng rãi: thà cửa sổ nhỏ hơn một chút còn hơn bị cắt mất đáy.
    const TASKBAR_H: f64 = 64.0;
    const EDGE: f64 = 40.0;
    const MIN_W: f64 = 900.0;
    const MIN_H: f64 = 520.0;

    let max_inner_w = (screen.width - EDGE - chrome_w).max(MIN_W);
    let max_inner_h = (screen.height - TASKBAR_H - EDGE - chrome_h).max(MIN_H);

    let w = inner.width.min(max_inner_w);
    let h = inner.height.min(max_inner_h);

    if w < inner.width - 1.0 || h < inner.height - 1.0 {
        let _ = win.set_size(LogicalSize::new(w, h));
    }
    // Căn giữa dù có thu nhỏ hay không: kích thước mặc định có thể đã đặt cửa sổ
    // lệch xuống dưới ngay từ đầu.
    let _ = win.center();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::ensure_dirs();
    // Đọc cài đặt ngay lúc khởi động để ngôn ngữ được áp trước khi bất kỳ
    // giá trị mặc định nào (tên nhóm file, mẫu dựng sẵn) được sinh ra.
    let s = config::load_settings();
    i18n::set_lang(s.lang);
    STATE.lock().unwrap().settings = Some(s);
    tauri::Builder::default()
        .setup(|app| {
            fit_window_to_screen(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_folders,
            pick_destination,
            check_path,
            check_destination,
            list_drives,
            scan_folders,
            make_plan,
            plan_page,
            plan_ids,
            run_preflight,
            apply_plan,
            cancel_run,
            history,
            interrupted,
            session_detail,
            undo,
            get_settings,
            set_settings,
            set_lang,
            confirm_lang,
            get_presets,
            get_catalog,
            default_profile,
            reveal,
            export_report,
        ])
        .run(tauri::generate_context!())
        .expect("khong khoi dong duoc ung dung");
}
