# TÀI LIỆU THIẾT KẾ PHẦN MỀM — PHIÊN BẢN 2

**Tên dự án:** Foldu
**Ngôn ngữ/Nền tảng:** Rust (lõi) + Tauri 2 + React 18 / TypeScript / Tailwind CSS
**Đóng gói:** Tauri bundler — 1 file `.exe` portable (~8–12 MB) + bản `.msi` cài đặt
**Trạng thái:** Thay thế toàn bộ bản đặc tả v1 (Python/PyQt6)

---

## 0. TÓM TẮT NHỮNG THAY ĐỔI SO VỚI V1

| # | Vấn đề trong v1 | Xử lý ở v2 |
|---|---|---|
| 1 | Không có bước xem trước — người dùng bấm là file bay ngay | **Preview Tree bắt buộc**: mô phỏng toàn bộ, hiện cây kết quả, cho tick/bỏ tick từng file rồi mới Apply |
| 2 | `history.json` chỉ ghi **sau khi** dọn xong → crash/mất điện giữa chừng = mất sạch khả năng Undo | **Write-Ahead Journal** trên SQLite, ghi từng thao tác **trước** khi thực hiện, tự phát hiện phiên dở dang khi mở lại app |
| 3 | Bỏ qua edge case Windows | Xử lý path >260 ký tự, tên cấm (CON/PRN/AUX), file bị khoá, khác ổ đĩa, OneDrive placeholder, symlink/junction |
| 4 | MD5 chậm và thừa | Lọc 3 tầng: kích thước → hash 8KB đầu+cuối → BLAKE3 toàn file (nhanh hơn MD5 ~5–10 lần, chạy đa luồng) |
| 5 | Chỉ 7 tiêu chí sắp xếp | **30 tiêu chí** chia 5 nhóm (xem §6) |
| 6 | Auto-Project chỉ so tiền tố, không hiểu tiếng Việt | Thuật toán v2: chuẩn hoá dấu, lọc từ nhiễu, gom cụm theo độ tương đồng, chống phân mảnh vụn |
| 7 | Chỉ có 3 tầng cứng | **Rules Engine** (if/then, chạy từ trên xuống) + Layers, dùng kết hợp |
| 8 | Chỉ MOVE | 5 chế độ: MOVE / COPY / **HARDLINK** (tạo "view" không tốn dung lượng) / SYMLINK / REPORT_ONLY |
| 9 | Không có gì cho người quản lý | Dashboard phân tích, chính sách lưu trữ, chạy tự động theo lịch/watcher, nhật ký kiểm toán, hồ sơ chuẩn dùng chung cho cả team |
| 10 | PyInstaller onefile khởi động 5–10 giây | Tauri khởi động <300ms, quét nhanh hơn 10–30 lần nhờ đa luồng |

---

## 1. TRIẾT LÝ THIẾT KẾ

Bốn nguyên tắc, thứ tự ưu tiên không đổi:

1. **Không bao giờ mất dữ liệu.** Không ghi đè, không xoá vĩnh viễn (chỉ vào Thùng rác), mọi thao tác đều hoàn tác được.
2. **Người dùng thấy trước khi làm.** Không có thao tác nào chạy mà không qua bản xem trước.
3. **Nhanh là một tính năng.** 100.000 file phải quét xong dưới 10 giây.
4. **Đẹp và dễ hiểu.** Người không rành máy tính vẫn dùng được ở chế độ Đơn giản; người chuyên sâu có Rules Engine.

---

## 2. LỰA CHỌN CÔNG NGHỆ

### 2.1. Vì sao đổi khỏi Python/PyQt6

| Tiêu chí | Python + PyQt6 | **Rust + Tauri** |
|---|---|---|
| Dung lượng file exe | 60–120 MB | **8–12 MB** |
| Thời gian khởi động | 5–10 giây (giải nén tạm) | **< 0.3 giây** |
| Tốc độ quét & hash | 1 luồng, chậm | **Đa luồng (rayon), nhanh 10–30×** |
| Giao diện đẹp | QSS hạn chế, khó làm hiện đại | **HTML/CSS đầy đủ, hiệu ứng mượt** |
| Bị antivirus nghi ngờ | Thường xuyên (PyInstaller) | Hiếm |
| Rủi ro crash mất dữ liệu | Cao (không kiểm soát type) | Thấp (Rust không có null/race) |

### 2.2. Ngăn xếp cụ thể

**Lõi (Rust):**
- `tauri` 2.x — khung ứng dụng
- `jwalk` — duyệt cây thư mục song song
- `rayon` — xử lý đa luồng
- `blake3` — băm nội dung file
- `rusqlite` (kèm `bundled`) — CSDL nhật ký, lịch sử, chỉ mục trùng lặp
- `serde` / `serde_json` — hồ sơ cấu hình
- `notify` — theo dõi thư mục thời gian thực
- `trash` — đưa file vào Thùng rác Windows
- `kamadak-exif` — đọc EXIF ảnh
- `lofty` — đọc tag nhạc (ID3/FLAC/M4A)
- `infer` — nhận dạng loại file theo magic bytes
- `unicode-normalization` + `deunicode` — chuẩn hoá tiếng Việt
- `windows` (crate) — API Windows: thuộc tính file, ADS, path dài
- `chrono` — xử lý thời gian

**Giao diện (TypeScript):**
- React 18 + Vite
- Tailwind CSS + **shadcn/ui** (bộ component đẹp sẵn, dark mode chuẩn)
- `@tanstack/react-virtual` — render cây 100.000 dòng không giật
- `framer-motion` — chuyển động mượt
- `recharts` — biểu đồ Dashboard
- `lucide-react` — icon

> **Phương án thay thế** nếu team quen .NET hơn: **C# .NET 8 + WPF/WinUI 3**. Cho chất lượng tương đương về hiệu năng và độ ổn định, chỉ thua ở dung lượng đóng gói và độ linh hoạt giao diện. Không chọn Electron (nặng 150MB+).

---

## 3. KIẾN TRÚC MODULE

```
src-tauri/src/
├── main.rs               # Khởi động, đăng ký lệnh Tauri
├── commands.rs           # Cầu nối IPC giữa React và Rust
│
├── scanner/
│   ├── walker.rs         # Duyệt cây song song, áp bộ lọc, chống symlink loop
│   ├── metadata.rs       # Đọc size/time/attributes/owner
│   └── probe.rs          # Đọc EXIF, tag nhạc, độ phân giải, magic bytes (lười — chỉ khi rule cần)
│
├── engine/
│   ├── rules.rs          # Rules Engine: điều kiện → hành động, chạy từ trên xuống
│   ├── layers.rs         # Sinh đường dẫn nhiều tầng
│   ├── criteria/         # 30 tiêu chí, mỗi tiêu chí 1 file, cùng trait `Criterion`
│   ├── clustering.rs     # Auto-Project v2
│   ├── bundling.rs       # Gom file đi kèm (RAW+JPG, mp4+srt)
│   └── planner.rs        # Sinh KẾ HOẠCH (Plan) — không đụng vào ổ đĩa
│
├── executor/
│   ├── journal.rs        # Write-Ahead Log trên SQLite
│   ├── mover.rs          # Thực thi: move/copy/hardlink, xử lý cross-volume
│   ├── collision.rs      # Xử lý đụng độ tên
│   ├── dedup.rs          # Lọc trùng lặp 3 tầng
│   └── undo.rs           # Hoàn tác toàn bộ / một phần / nhiều phiên
│
├── policy/
│   ├── watcher.rs        # Theo dõi thư mục, tự dọn khi có file mới
│   ├── scheduler.rs      # Chạy theo lịch
│   └── retention.rs      # Chính sách lưu trữ / nén / dọn kho
│
├── analytics/
│   ├── report.rs         # Thống kê trước-sau, xuất HTML/CSV
│   └── audit.rs          # Nhật ký kiểm toán
│
├── safety/
│   ├── paths.rs          # Path dài, tên cấm, sanitize, kiểm tra vòng lặp
│   ├── guards.rs         # Danh sách thư mục cấm động vào
│   └── preflight.rs      # Kiểm tra trước khi chạy: dung lượng, quyền, file khoá
│
└── store/
    ├── db.rs             # Schema SQLite + migration
    └── profiles.rs       # Đọc/ghi/xuất/nhập hồ sơ cấu hình
```

**Nguyên tắc kiến trúc quan trọng nhất:** `engine/` **tuyệt đối không được ghi vào ổ đĩa**. Nó chỉ nhận danh sách file và trả về một `Plan` (danh sách các thao tác dự kiến). Chỉ `executor/` mới được phép ghi. Nhờ tách bạch này, Preview và Apply dùng chung một mã nguồn — cái người dùng thấy chính xác là cái sẽ xảy ra.

---

## 4. MÔ HÌNH DỮ LIỆU (SQLite)

Thay `history.json` bằng SQLite — vừa an toàn khi crash (WAL mode), vừa cho phép truy vấn thống kê.

```sql
-- Mỗi lần bấm Apply = 1 session
CREATE TABLE sessions (
    id           INTEGER PRIMARY KEY,
    started_at   INTEGER NOT NULL,
    finished_at  INTEGER,                    -- NULL = phiên dở dang (crash)
    profile_name TEXT,
    source_paths TEXT NOT NULL,              -- JSON array
    mode         TEXT NOT NULL,              -- MOVE|COPY|HARDLINK|SYMLINK|REPORT
    total_files  INTEGER,
    total_bytes  INTEGER,
    status       TEXT NOT NULL               -- RUNNING|DONE|FAILED|UNDONE|PARTIAL
);

-- Write-Ahead Journal: ghi TRƯỚC khi động vào file
CREATE TABLE moves (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER NOT NULL REFERENCES sessions(id),
    seq         INTEGER NOT NULL,
    old_path    TEXT NOT NULL,
    new_path    TEXT NOT NULL,
    size        INTEGER,
    hash        TEXT,
    state       TEXT NOT NULL,               -- PLANNED|DONE|FAILED|UNDONE|SKIPPED
    error       TEXT,
    reason      TEXT                         -- rule/tiêu chí nào quyết định đích này
);
CREATE INDEX idx_moves_session ON moves(session_id, seq);

-- Chỉ mục hash để dò trùng lặp nhanh giữa các phiên
CREATE TABLE file_index (
    path        TEXT PRIMARY KEY,
    size        INTEGER NOT NULL,
    mtime       INTEGER NOT NULL,
    quick_hash  TEXT,                        -- 8KB đầu + 8KB cuối
    full_hash   TEXT,                        -- BLAKE3 toàn file
    indexed_at  INTEGER NOT NULL
);
CREATE INDEX idx_index_size ON file_index(size);
CREATE INDEX idx_index_hash ON file_index(full_hash);

-- Nhật ký kiểm toán (không bao giờ xoá)
CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY,
    ts         INTEGER NOT NULL,
    actor      TEXT,                         -- tên user Windows
    action     TEXT NOT NULL,
    detail     TEXT
);
```

**Quy trình chống crash:**
1. Ghi `sessions` với `status = RUNNING`.
2. Ghi toàn bộ `moves` với `state = PLANNED`.
3. Với từng file: thực hiện thao tác → cập nhật `state = DONE`.
4. Kết thúc: `status = DONE`.
5. **Khi mở app**, nếu tìm thấy session `RUNNING` → hiện banner: *"Phiên làm việc lúc 14:32 ngày 26/07 chưa hoàn tất. Bạn muốn tiếp tục hay hoàn tác phần đã làm?"*

---

## 5. ENGINE SẮP XẾP: RULES + LAYERS

### 5.1. Hai chế độ, một engine

**Chế độ Đơn giản (mặc định)** — dành cho 90% người dùng: chọn tối đa 4 tầng từ danh sách 30 tiêu chí, kéo thả để đổi thứ tự. Kèm 8 **mẫu dựng sẵn** bấm 1 phát chạy ngay:

| Mẫu | Cấu trúc sinh ra |
|---|---|
| Dọn Downloads | `Loại file / Thời gian tương đối` |
| Thư viện ảnh | `Năm / Tháng / Sự kiện (cụm ảnh liên tiếp)` |
| Kho dự án | `Dự án tự động / Loại file` |
| Kho tài liệu công ty | `Năm / Quý / Từ khoá nghiệp vụ` |
| Dọn ổ cứng đầy | `Bậc kích thước / Loại file` — kèm báo cáo lãng phí |
| Thư viện media | `Loại / Độ phân giải / Năm` |
| Lưu trữ dài hạn | `Độ nguội truy cập / Năm` |
| Chỉ tìm trùng lặp | Không di chuyển, chỉ xuất báo cáo |

**Chế độ Nâng cao** — Rules Engine, mạnh như Hazel (macOS) hay File Juggler:

```json
{
  "name": "Chuẩn thư mục phòng Kinh doanh 2026",
  "version": 2,
  "mode": "MOVE",
  "rules": [
    {
      "name": "Hợp đồng đã ký giữ nguyên tên, vào kho pháp lý",
      "match": "ALL",
      "conditions": [
        { "field": "name",      "op": "contains_any", "value": ["hop dong", "contract", "HĐ"], "ignore_diacritics": true },
        { "field": "extension", "op": "in",           "value": ["pdf", "docx"] }
      ],
      "layers": ["LITERAL:01-Phap-Ly/Hop-Dong", "TIME_MODIFIED:%Y"],
      "stop": true
    },
    {
      "name": "Hoá đơn theo quý",
      "match": "ANY",
      "conditions": [
        { "field": "name", "op": "regex", "value": "(?i)(hoa\\s?don|invoice|VAT)" }
      ],
      "layers": ["LITERAL:02-Ke-Toan", "TIME_FISCAL_QUARTER"],
      "stop": true
    },
    {
      "name": "File tạm quá 30 ngày -> Thùng rác",
      "match": "ALL",
      "conditions": [
        { "field": "extension",  "op": "in",       "value": ["tmp", "crdownload", "part"] },
        { "field": "age_days",   "op": "greater",  "value": 30 }
      ],
      "action": "RECYCLE",
      "stop": true
    },
    {
      "name": "Mặc định",
      "match": "ALL",
      "conditions": [],
      "layers": ["AUTO_PROJECT", "TYPE"]
    }
  ],
  "filters": {
    "include_hidden": false,
    "include_system": false,
    "skip_cloud_placeholder": true,
    "min_size_bytes": 0,
    "ignore_patterns": ["**/node_modules/**", "**/.git/**", "desktop.ini", "Thumbs.db"]
  },
  "safety": {
    "keep_sidecar_together": true,
    "treat_project_folders_as_unit": true,
    "max_new_folders": 500
  }
}
```

**Toán tử điều kiện:** `equals`, `contains`, `contains_any`, `starts_with`, `ends_with`, `regex`, `in`, `greater`, `less`, `between`, `is_empty`, `matches_glob`. Mọi toán tử chuỗi hỗ trợ cờ `ignore_diacritics` (bỏ dấu tiếng Việt) và `ignore_case`.

**Trường điều kiện:** `name`, `extension`, `real_type`, `size`, `age_days`, `created`, `modified`, `accessed`, `taken`, `depth`, `parent_folder`, `full_path`, `owner`, `download_source`, `is_duplicate`, `width`, `height`, `duration`, `camera`, `artist`, `album`, `page_count`, `attribute`.

### 5.2. Chế độ thao tác

| Chế độ | Mô tả | Dùng khi |
|---|---|---|
| `MOVE` | Di chuyển thật | Dọn dẹp bình thường |
| `COPY` | Sao chép, giữ nguyên bản gốc | Sợ hỏng, hoặc gom từ ổ mạng |
| **`HARDLINK`** | Tạo cấu trúc mới bằng hard link — **không tốn thêm 1 byte nào**, file gốc vẫn nguyên chỗ cũ | Muốn "xem thư viện theo cách khác" mà không phá cấu trúc gốc. Cực mạnh cho kho ảnh/media |
| `SYMLINK` | Như trên nhưng liên kết mềm, chạy được qua ổ khác | Kho lớn trải nhiều ổ |
| `REPORT_ONLY` | Không đụng gì, chỉ xuất báo cáo | Kiểm toán, khảo sát trước |

Bổ sung 2 thao tác đảo chiều:
- **Flatten (Gỡ phẳng):** kéo toàn bộ file từ mọi thư mục con lên 1 tầng — để làm lại từ đầu.
- **Reorganize (Sắp xếp lại):** đọc cấu trúc hiện có, áp bộ tiêu chí mới, tính đường đi ngắn nhất (chỉ di chuyển file thực sự cần đổi chỗ).

---

## 6. DANH MỤC 30 TIÊU CHÍ SẮP XẾP

### Nhóm A — Thuộc tính cơ bản
| Mã | Tên | Ghi chú |
|---|---|---|
| `NONE` | Bỏ qua tầng | |
| `TYPE` | Nhóm loại file | Đọc từ cấu hình, mặc định 9 nhóm |
| `EXT` | Đuôi file cụ thể | JPG, PDF… không gộp |
| `REAL_TYPE` | **Loại thật theo magic bytes** | Bắt file bị đổi đuôi sai (`.jpg` thực ra là `.exe`) — vừa để phân loại đúng, vừa là cảnh báo bảo mật |
| `SIZE_TIER` | Bậc kích thước | Ngưỡng cố định **hoặc theo phân vị động** (25%/50%/75% của chính tập file đang quét — hợp lý hơn ngưỡng cứng 1GB) |
| `ALPHABET` | Chữ cái đầu | Chuẩn hoá tiếng Việt: `Ánh`→`A`, `Đông`→`Đ`. Gộp `0-9` và `#` cho ký tự lạ |

### Nhóm B — Thời gian
| Mã | Tên | Ghi chú |
|---|---|---|
| `TIME_MODIFIED` | Ngày sửa | Định dạng tuỳ ý (`%Y/%m`), có `/` thì tự sinh tầng lồng |
| `TIME_CREATED` | Ngày tạo | |
| `TIME_TAKEN` | **Ngày chụp thật (EXIF)** | Fallback về `modified` nếu không có EXIF. Đây mới là mốc đúng cho ảnh — copy qua lại làm hỏng `modified` |
| `TIME_RELATIVE` | Tương đối | `Hom-Nay` / `7-Ngay-Qua` / `Thang-Nay` / `Quy-Nay` / `Nam-Nay` / `Cu-Hon-1-Nam` |
| `TIME_QUARTER` | Quý | `2026-Q1` |
| `TIME_FISCAL_QUARTER` | **Quý tài chính** | Cho phép đặt tháng bắt đầu năm tài chính (VD: tháng 4) — quan trọng với kế toán |
| `TIME_WEEK` | Tuần ISO | `2026-W31` |
| `ACCESS_HEAT` | **Độ nguội truy cập** | `Nong` (<30 ngày) / `Am` (<180) / `Nguoi` (<365) / `Dong-Bang` (>365). Nền tảng cho chính sách lưu trữ |

### Nhóm C — Nội dung & metadata
| Mã | Tên | Ghi chú |
|---|---|---|
| `EXIF_CAMERA` | Máy ảnh / điện thoại | `iPhone-15-Pro`, `Canon-EOS-R6` |
| `EXIF_GPS_PLACE` | **Nơi chụp** | Reverse geocode offline (bộ dữ liệu thành phố ~2MB nhúng sẵn) → `Da-Nang`, `Tokyo` |
| `MEDIA_RESOLUTION` | Độ phân giải | `8K` / `4K` / `1080p` / `720p` / `Thap` |
| `MEDIA_DURATION` | Độ dài | `Duoi-1-Phut` / `1-10-Phut` / `Tren-10-Phut` |
| `IMAGE_ORIENTATION` | Hướng ảnh | `Ngang` / `Doc` / `Vuong` — dân thiết kế rất cần |
| `SCREENSHOT_DETECT` | **Tách ảnh chụp màn hình** | Nhận theo tên (`Screenshot`, `Ảnh chụp màn hình`), thiếu EXIF camera, và tỉ lệ khớp độ phân giải màn hình. Tách khỏi ảnh thật giúp thư viện ảnh sạch hẳn |
| `AUDIO_TAG` | Nghệ sĩ / Album | Đọc ID3, FLAC, M4A |
| `DOC_AUTHOR` | Tác giả tài liệu | Từ metadata Office/PDF |
| `DOC_PAGES` | Số trang | `1-Trang` / `Ngan` / `Dai` |

### Nhóm D — Ngữ nghĩa & quan hệ (phần "hay ho" nhất)
| Mã | Tên | Ghi chú |
|---|---|---|
| `AUTO_PROJECT` | **Gom cụm dự án tự động v2** | Xem §7 |
| `KEYWORD_RULE` | Bảng từ khoá nghiệp vụ | `hop dong|contract` → `Hop-Dong`. Hỗ trợ regex, bỏ dấu. Người dùng tự soạn bảng |
| `VERSION_GROUP` | **Gom phiên bản** | Nhận `v1/v2/final/final-2/copy/Bản sao/(1)` → gom về 1 thư mục, **đánh dấu bản mới nhất** và tạo shortcut `_MOI-NHAT.lnk`. Giải quyết nỗi đau "file final_final_v3 thật" |
| `SEQUENCE_BURST` | **Chuỗi sự kiện** | `IMG_0001..0247` liên tiếp và chụp cách nhau <4 giờ → cùng một sự kiện → `Su-Kien-2026-03-15` |
| `SIDECAR_BUNDLE` | **Giữ file đi kèm cùng nhau** | RAW+JPG, `.mp4`+`.srt`, `.psd`+preview, `.obj`+`.mtl`+texture. **Đây là ràng buộc, không phải tầng** — bật lên là mọi tiêu chí khác phải tôn trọng |
| `FOLDER_UNIT` | **Thư mục dự án là một khối** | Thấy `.git`, `package.json`, `.sln`, `node_modules` → coi cả thư mục là 1 đơn vị, **không phá tung ra**. Không có cái này thì sắp xếp một thư mục code = phá nát dự án |
| `DOWNLOAD_SOURCE` | **Nguồn tải về** | Đọc Alternate Data Stream `Zone.Identifier` của Windows → biết file tải từ tên miền nào → `Tai-Tu-example.com`. Cực hiệu quả cho thư mục Downloads |
| `LANGUAGE_SCRIPT` | Ngôn ngữ tên file | `Tieng-Viet` / `English` / `中文` / `日本語` |
| `AI_SEMANTIC` | Phân loại theo chủ đề (v2.0) | Tuỳ chọn: gọi mô hình cục bộ hoặc API để đọc tên file + vài KB nội dung → gán chủ đề. Mặc định **tắt**, phải bật thủ công vì có gửi dữ liệu ra ngoài |

### Nhóm E — Hệ thống
| Mã | Tên | Ghi chú |
|---|---|---|
| `OWNER` | Chủ sở hữu file | Hữu ích trên ổ mạng dùng chung |
| `ORIGIN_FOLDER` | Giữ dấu vết thư mục gốc | `Tu-Desktop`, `Tu-Downloads` — để còn nhớ đường về |
| `LITERAL:<text>` | Thư mục cố định | Dùng trong Rules để ghim đường dẫn |

---

## 7. AUTO-PROJECT CLUSTERING V2

Thuật toán v1 (cắt token → cụm tiền tố xuất hiện ≥2 lần → lấy cụm dài nhất) có 4 lỗ hổng: không hiểu tiếng Việt có dấu, không lọc từ nhiễu, ngưỡng 2 quá thấp gây phân mảnh vụn, và chỉ so tiền tố nên bỏ sót file cùng dự án nhưng khác thứ tự từ.

### Quy trình v2

**Bước 1 — Chuẩn hoá.**
Bỏ đuôi file → NFC normalize → **bỏ dấu tiếng Việt** (`Báo cáo` → `bao cao`) → về chữ thường → tách token theo ` `, `-`, `_`, `.`, `+`, và ranh giới camelCase (`BaoCaoThang` → `bao|cao|thang`).

**Bước 2 — Lọc từ nhiễu.** Loại khỏi việc đặt tên cụm (nhưng vẫn giữ trong tên file):
- Số phiên bản: `v1 v2 ver final draft cuoi ban sao copy new moi old cu`
- Tiền tố máy ảnh: `img dsc dcim mvi vid pxl photo screenshot anh chup man hinh`
- Ngày tháng thuần số: `20260315`, `2026-03-15`, `15032026`
- Số thứ tự đơn lẻ, chuỗi hex/GUID, chuỗi <2 ký tự
- Từ dừng: `the a of and cua va` …
- Danh sách này **cho người dùng sửa được** trong Cài đặt.

**Bước 3 — Sinh ứng viên cụm.** N-gram liên tiếp độ dài 1–5 token (thay vì chỉ tiền tố, quét cả n-gram bên trong tên).

**Bước 4 — Chấm điểm.** Mỗi cụm ứng viên tính:

```
score = count × log2(1 + token_len) × idf_bonus × position_bonus
```
- `count`: số file chứa cụm
- `token_len`: số token của cụm (cụm dài mô tả tốt hơn)
- `idf_bonus`: phạt cụm quá phổ biến (xuất hiện ở >60% tổng file thì gần như vô nghĩa)
- `position_bonus`: ×1.5 nếu cụm nằm ở đầu tên file

**Bước 5 — Chọn cụm hợp lệ.** Ngưỡng động thay vì cứng "≥2":
```
min_count = max(3, ceil(sqrt(tổng_số_file) / 2))
```
Với 25 file → cần ≥3; với 400 file → cần ≥10. Người dùng chỉnh được qua thanh trượt **"Độ mịn: Gộp nhiều ← → Chia nhỏ"**, kết quả cập nhật preview theo thời gian thực.

**Bước 6 — Gom cụm mờ (fuzzy merge).** Hai cụm có độ tương đồng Jaccard token ≥0.7 hoặc khoảng cách Levenshtein chuẩn hoá ≤0.15 thì gộp làm một (`bao cao thang 10` và `baocao thang 10` là một). Giải quyết lỗi gõ tay.

**Bước 7 — Gán file & chống phân mảnh.**
- Mỗi file lấy cụm có điểm cao nhất mà nó chứa.
- Sau khi gán, **cụm nào có <3 file thì giải tán**, file chuyển sang cụm tốt thứ hai hoặc vào `_Khac`.
- Nếu tổng số thư mục sinh ra vượt `max_new_folders` (mặc định 500) → tự nâng ngưỡng và tính lại, đồng thời cảnh báo trên UI.
- Tên thư mục cuối cùng lấy **dạng gốc có dấu phổ biến nhất** trong nhóm (`Báo-Cáo-Tháng-10`), không phải dạng đã bỏ dấu.

**Bước 8 — Sanitize.** Tên thư mục sinh ra phải qua `safety::paths::sanitize()` — xem §8.2.

---

## 8. AN TOÀN DỮ LIỆU

### 8.1. Danh sách chặn cứng (không cho phép chạy)

Từ chối thẳng, không cho người dùng ghi đè:
- Gốc ổ đĩa (`C:\`, `D:\`)
- `C:\Windows`, `C:\Program Files`, `C:\Program Files (x86)`, `C:\ProgramData`
- `%USERPROFILE%\AppData`
- Thư mục đích nằm bên trong thư mục nguồn theo cách gây đệ quy vô hạn
- Ổ đĩa mạng khi chế độ `MOVE` mà chưa bật cờ xác nhận riêng

Cảnh báo mạnh (cho phép nhưng phải gõ xác nhận): toàn bộ Desktop, toàn bộ Documents, thư mục có >50.000 file.

### 8.2. Xử lý đường dẫn Windows

| Vấn đề | Cách xử lý |
|---|---|
| Path > 260 ký tự | Dùng tiền tố `\\?\` cho mọi lời gọi hệ thống. Nếu đường dẫn đích vượt 32.767 → rút ngắn tên thư mục và cảnh báo |
| Ký tự cấm trong tên thư mục sinh ra | Thay `< > : " / \ | ? *` và ký tự điều khiển bằng `-` |
| Tên thiết bị cấm | `CON PRN AUX NUL COM1-9 LPT1-9` → thêm hậu tố `_` |
| Kết thúc bằng dấu chấm hoặc khoảng trắng | Cắt bỏ |
| Tên rỗng sau khi làm sạch | Thay bằng `_Khong-Ten` |
| Trùng tên thư mục do khác dấu | Windows không phân biệt hoa/thường → so sánh case-insensitive khi gom |

### 8.3. Kiểm tra trước khi chạy (Preflight)

Chạy tự động ngay trước khi Apply, chặn nếu có lỗi đỏ:
1. **Dung lượng trống** — với `COPY` hoặc move khác ổ đĩa, cần đủ chỗ + 10% dự phòng.
2. **File đang bị khoá** — thử mở với quyền ghi; file đang mở trong Word/Photoshop sẽ bị đánh dấu SKIP, báo rõ tên chương trình đang giữ nếu lấy được.
3. **Quyền ghi** — thử tạo file tạm ở đích.
4. **File đám mây chưa tải về (OneDrive/Google Drive placeholder)** — nhận diện qua thuộc tính `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` / `FILE_ATTRIBUTE_OFFLINE`. **Mặc định BỎ QUA** — vì chỉ cần đọc là kích hoạt tải hàng trăm GB về máy. Đây là cái bẫy mà hầu hết phần mềm cùng loại mắc phải.
5. **Symlink / Junction** — không đi theo khi duyệt (chống vòng lặp vô hạn), không di chuyển đích.
6. **File hệ thống / ẩn** — mặc định bỏ qua.
7. **Ước lượng thời gian** dựa trên tổng dung lượng và tốc độ ổ đĩa đo thử.

### 8.4. Thực thi an toàn

- **Cùng ổ đĩa:** dùng `rename` (tức thời, nguyên tử).
- **Khác ổ đĩa:** copy → **kiểm tra hash bản đích** → chỉ khi khớp mới xoá bản nguồn. Nếu copy dở dang bị ngắt → xoá file rác ở đích, giữ nguyên nguồn.
- **Giữ nguyên timestamp** (`created`, `modified`, `accessed`) sau khi di chuyển.
- **Không bao giờ `delete`** — mọi thao tác xoá đều đưa vào **Thùng rác Windows** qua crate `trash`.
- **Có nút Tạm dừng / Huỷ** — huỷ giữa chừng sẽ dừng sạch, phần đã làm vẫn hoàn tác được đầy đủ.
- Xử lý tuần tự có kiểm soát để không làm nghẽn I/O; hash chạy song song.

### 8.5. Hệ thống hoàn tác

Nâng cấp từ "undo phiên gần nhất" thành:
- **Undo nhiều phiên** — ngăn xếp lịch sử, hoàn tác ngược từng phiên.
- **Undo chọn lọc** — chỉ hoàn tác một số file/thư mục được tick trong danh sách lịch sử.
- **Undo phiên dở dang** — tự phát hiện khi mở app sau crash.
- **Redo** — làm lại phiên vừa hoàn tác.
- Nếu file đã bị người dùng đổi tên/di chuyển sau đó → không mù quáng ghi đè, mà báo xung đột và hỏi.
- Sau khi Undo: chạy `remove_empty_folders` (duyệt hậu thứ tự) — **nhưng chỉ xoá thư mục do chính phần mềm tạo ra trong phiên đó**, không đụng thư mục rỗng vốn có từ trước. (Lỗi này v1 mắc phải.)

---

## 9. XỬ LÝ TRÙNG LẶP V2

### 9.1. Lọc 3 tầng (nhanh hơn v1 rất nhiều)

1. **Tầng 1 — Kích thước.** Nhóm theo `size`. Nhóm chỉ 1 file → chắc chắn không trùng, dừng. Loại được ~95% file mà không đọc byte nào.
2. **Tầng 2 — Quick hash.** BLAKE3 của 8KB đầu + 8KB cuối. Loại tiếp ~99% phần còn lại.
3. **Tầng 3 — Full hash.** BLAKE3 toàn file, chạy song song đa luồng. Chỉ đến bước này với các file thực sự nghi ngờ.

Kết quả lưu vào `file_index` để lần chạy sau không phải hash lại (kiểm tra `mtime` + `size` để biết cache còn hợp lệ).

### 9.2. Chiến lược giữ file

Không còn "ném hết vào thư mục cách ly" như v1. Cho người dùng chọn:

| Chiến lược | Mô tả |
|---|---|
| Giữ bản cũ nhất | Thường là bản gốc |
| Giữ bản mới nhất | |
| Giữ bản có đường dẫn ngắn nhất | Thường là bản "chính chủ", không phải bản copy |
| Giữ bản có tên sạch nhất | Loại bản có `(1)`, `- Copy`, `Bản sao` |
| Giữ bản ở thư mục ưu tiên | Người dùng chỉ định thư mục "nguồn chân lý" |
| **Thay bản thừa bằng hard link** | Giải phóng dung lượng nhưng file vẫn nằm nguyên chỗ cũ, mở vẫn được. An toàn nhất |
| Hỏi từng nhóm | Có xem trước ảnh thumbnail |

Bản thừa mặc định vào **Thùng rác**, không xoá vĩnh viễn. Xuất kèm báo cáo `Bao-Cao-Trung-Lap.html` có thumbnail, đường dẫn, dung lượng tiết kiệm được.

### 9.3. Chỉ trùng tên, khác nội dung

Đổi tên bằng hậu tố tăng dần `filename (1).ext` — nhưng **không dùng đệ quy** (v1 dùng đệ quy, dễ tràn stack với thư mục có hàng nghìn file trùng tên). Dùng vòng lặp + truy vấn tập tên đã tồn tại trong bộ nhớ. **Tuyệt đối không ghi đè.**

### 9.4. Trùng lặp gần đúng (v2.0)

Cho ảnh: perceptual hash (dHash/pHash) → phát hiện cùng một ảnh nhưng khác kích thước, khác nén, đã crop nhẹ. Đây là thứ tiết kiệm dung lượng nhiều nhất trong thư viện ảnh thật.

---

## 10. CHỨC NĂNG CẤP QUẢN LÝ

Đây là phần v1 hoàn toàn không có, và cũng là phần tạo ra giá trị lớn nhất khi triển khai cho một tổ chức.

### 10.1. Dashboard phân tích (chạy trước khi dọn)

Quét xong là hiện ngay, trước cả khi quyết định dọn thế nào:
- **Treemap dung lượng** — nhìn phát biết thư mục nào phình to
- **Top 20 file lớn nhất** — bấm là mở vị trí
- **Dung lượng lãng phí do trùng lặp** — *"Bạn đang lãng phí 4,2 GB vì 1.847 file trùng lặp"*
- **Phân bố theo loại file** (biểu đồ tròn) và **theo năm** (biểu đồ cột)
- **File nguội** — *"312 GB chưa được mở trong hơn 1 năm"*
- **Điểm sức khoẻ thư mục (0–100)** — tính từ độ sâu trung bình, tỉ lệ file ở gốc, tỉ lệ trùng lặp, tỉ lệ file vô danh (`Untitled`, `New Document`, `IMG_xxxx`)
- **So sánh trước/sau** khi dọn xong

### 10.2. Chính sách lưu trữ (Retention Policy)

Định nghĩa vòng đời file, chạy tự động theo lịch:

```
Chưa mở > 180 ngày   →  chuyển vào  \_Kho-Luu-Tru\{Năm}\
Trong kho > 365 ngày →  nén thành ZIP theo năm
Trong ZIP > 3 năm    →  cảnh báo, chờ duyệt tay mới đưa vào Thùng rác
```

Đây là thứ biến phần mềm từ "công cụ dọn dẹp một lần" thành "hệ thống quản trị dữ liệu".

### 10.3. Chế độ tự động

- **Watcher** — theo dõi thư mục (mặc định gợi ý Downloads, Desktop). File mới xuất hiện, ổn định >5 giây → áp hồ sơ đã chọn → hiện thông báo nhỏ *"Đã chuyển hoadon-t7.pdf vào Kế toán/2026-Q3"* kèm nút **Hoàn tác**.
- **Scheduler** — chạy theo lịch (hằng ngày 22:00, thứ Hai hằng tuần…), tích hợp Task Scheduler của Windows.
- **Chạy khi cắm USB** — nhận diện thẻ nhớ máy ảnh, tự nhập ảnh theo cấu trúc `Năm/Sự kiện`.
- Mọi chế độ tự động đều **bắt buộc bật Preflight** và ghi journal đầy đủ.

### 10.4. Hồ sơ chuẩn dùng chung cho team

- Xuất hồ sơ ra file `.foldu.json` (kèm chữ ký kiểm tra tính toàn vẹn) → gửi cho cả phòng → mọi người dùng chung một chuẩn thư mục.
- Trỏ đến một file hồ sơ trên ổ mạng → tự đồng bộ khi có cập nhật.
- **Chế độ kiểm tra tuân thủ:** chạy `REPORT_ONLY` trên thư mục của một phòng ban, xuất báo cáo *"73% file đúng chuẩn, 412 file sai vị trí"* — không đụng vào file của ai.

### 10.5. Chuẩn hoá tên file (Rename Engine)

Đổi tên hàng loạt theo mẫu, chạy chung một luồng preview/undo với việc sắp xếp:
```
{ngay:%Y%m%d}_{du_an}_{stt:000}{ext}
```
Kèm các phép biến đổi: bỏ dấu tiếng Việt, chuyển kebab-case/snake_case, cắt khoảng trắng thừa, bỏ `- Copy`/`Bản sao`, cắt tên quá dài, tìm-thay bằng regex. Đây là công cụ quản trị quan trọng — tên file chuẩn thì tìm kiếm mới hiệu quả.

### 10.6. Báo cáo & kiểm toán

- Xuất **HTML** (đẹp, có biểu đồ, gửi sếp được) hoặc **CSV** (đưa vào Excel).
- Nội dung: ai chạy, lúc nào, hồ sơ nào, bao nhiêu file, bao nhiêu GB, danh sách đầy đủ nguồn→đích, lý do (rule nào quyết định), file bị bỏ qua và vì sao.
- `audit_log` không bao giờ bị xoá, kể cả khi Undo.

### 10.7. Chế độ dòng lệnh (CLI)

```bash
foldu.exe run --profile "Chuan-Kinh-Doanh.foldu.json" --source "D:\Data" --dry-run
foldu.exe run --profile ... --source ... --apply --report out.csv
foldu.exe undo --session 42
foldu.exe scan --source "D:\Data" --report-only --format json
```
Cho phép đưa vào script, chạy trên máy chủ file, tích hợp CI/backup.

---

## 11. GIAO DIỆN NGƯỜI DÙNG

### 11.1. Hệ thống thiết kế

- **Bố cục:** cột trái điều hướng (thu gọn được) + vùng nội dung chính. Không dùng tab ngang kiểu cũ.
- **Màu:** nền `zinc-950` (tối) / `zinc-50` (sáng), màu nhấn `indigo-500`. Ngữ nghĩa: xanh lá = thêm, hổ phách = đổi tên, đỏ = xoá/thùng rác, xám = bỏ qua.
- **Chữ:** Inter (giao diện) + JetBrains Mono (đường dẫn, dung lượng). Không dùng font hệ thống mặc định.
- **Bo góc** 8px, **đổ bóng** rất nhẹ, **chuyển động** 150–250ms ease-out. Không hiệu ứng loè loẹt.
- **Dark mode** là mặc định, có nút chuyển sáng/tối và theo hệ thống.
- **Song ngữ** Việt/Anh, chuyển ngay không cần khởi động lại.
- **Truy cập bàn phím** đầy đủ: `Ctrl+O` mở thư mục, `Space` bật/tắt tick, `Ctrl+Enter` áp dụng, `Ctrl+Z` hoàn tác, `/` tìm kiếm.

### 11.2. Các màn hình

**① Trang chủ / Thả file**
Vùng kéo-thả lớn ở giữa. Bên dưới: danh sách thư mục dùng gần đây kèm thống kê nhanh. Nhận nhiều thư mục nguồn cùng lúc (gom từ nhiều nơi về một đích).

**② Phân tích (tự hiện sau khi quét)**
Dashboard §10.1. Có nút **"Đề xuất cách sắp xếp"** — phần mềm nhìn vào đặc điểm dữ liệu (nhiều ảnh? nhiều tài liệu? nhiều bản trùng?) rồi tự gợi ý 3 phương án kèm mô tả kết quả.

**③ Thiết kế cấu trúc**
- Bên trái: 8 mẫu dựng sẵn + khu vực xếp tầng (kéo-thả thẻ tiêu chí, tối đa 4 tầng).
- Bên phải: **cây kết quả cập nhật trực tiếp** ngay khi kéo thả — chưa đụng ổ đĩa.
- Thanh trượt "Độ mịn" cho Auto-Project.
- Nút chuyển sang chế độ Rules (soạn thảo có gợi ý và kiểm tra cú pháp).

**④ Xem trước (màn hình quan trọng nhất)**
- **Bố cục 2 cột:** trái = cây thư mục kết quả sẽ có; phải = danh sách file, mỗi dòng `tên file → đường dẫn đích mới` kèm huy hiệu lý do (`Rule: Hoá đơn`, `AUTO_PROJECT`, `TRÙNG LẶP`).
- **Ảo hoá danh sách** — 100.000 dòng vẫn cuộn mượt.
- **Tick/bỏ tick từng file hoặc cả nhóm.** Sửa tay đường dẫn đích của bất kỳ file nào.
- **Lọc nhanh:** chỉ hiện file trùng lặp / file sẽ bị đổi tên / file bị bỏ qua.
- **Thanh cảnh báo màu hổ phách** liệt kê mọi phát hiện của Preflight: *"12 file đang mở trong ứng dụng khác sẽ bị bỏ qua"*, *"3 file trên OneDrive chưa tải về, đã bỏ qua"*.
- **Dải tóm tắt cuối màn hình:** `1.284 di chuyển · 47 đổi tên · 213 trùng lặp (4,2 GB) · 12 bỏ qua · ước tính 1 phút 40 giây`.
- Nút **Áp dụng** to, rõ, và chỉ sáng lên sau khi Preflight xanh.

**⑤ Đang chạy**
Thanh tiến trình kèm tên file hiện tại, tốc độ MB/s, thời gian còn lại. Nút **Tạm dừng** và **Huỷ**. Nhật ký cuộn theo thời gian thực.

**⑥ Hoàn tất**
So sánh trước/sau, số liệu tiết kiệm được, nút **Mở thư mục**, **Xuất báo cáo**, và nút **Hoàn tác** to đặt ngay đó.

**⑦ Lịch sử**
Bảng các phiên đã chạy, mở rộng xem chi tiết từng file, hoàn tác toàn bộ hoặc chọn lọc, xuất báo cáo bất kỳ phiên nào.

**⑧ Cài đặt**
Nhóm loại file, bảng từ khoá, danh sách từ nhiễu, mẫu bỏ qua, ngưỡng an toàn, hồ sơ (xuất/nhập/đồng bộ), tự động hoá, ngôn ngữ, giao diện.

---

## 12. YÊU CẦU HIỆU NĂNG

| Chỉ tiêu | Mục tiêu |
|---|---|
| Khởi động ứng dụng | < 300 ms |
| Quét 100.000 file (chỉ metadata) | < 10 giây |
| Sinh kế hoạch cho 100.000 file | < 3 giây |
| Hash 10 GB (SSD, 8 luồng) | < 30 giây |
| Bộ nhớ với 500.000 file | < 500 MB |
| Cuộn danh sách preview 100.000 dòng | 60 fps |
| Kích thước file cài đặt | < 15 MB |

Đọc metadata nặng (EXIF, tag nhạc, độ phân giải) **chỉ thực hiện khi có tiêu chí/rule thực sự cần** — không quét mù toàn bộ.

---

## 13. LỘ TRÌNH PHÁT TRIỂN

**Giai đoạn 1 — MVP (bản dùng được thật)**
Quét · Layers cơ bản (TYPE, EXT, TIME_*, SIZE_TIER, ALPHABET, AUTO_PROJECT v2) · Preview Tree · Journal + Undo · Xử lý trùng lặp 3 tầng · Toàn bộ §8 An toàn · Giao diện màn hình ①③④⑤⑥⑦.

**Giai đoạn 2 — Bản 1.0**
Rules Engine · Dashboard phân tích · 30 tiêu chí đầy đủ · SIDECAR_BUNDLE + FOLDER_UNIT · Rename Engine · Hồ sơ xuất/nhập · Báo cáo HTML/CSV · Chế độ HARDLINK.

**Giai đoạn 3 — Bản 1.5**
Watcher + Scheduler · Chính sách lưu trữ · CLI · Chế độ kiểm tra tuân thủ · Song ngữ.

**Giai đoạn 4 — Bản 2.0**
Trùng lặp gần đúng cho ảnh (pHash) · AI_SEMANTIC · EXIF_GPS_PLACE · Bản macOS/Linux (Tauri cho gần như miễn phí).

---

## 14. TIÊU CHÍ NGHIỆM THU

Không được phát hành nếu chưa vượt qua toàn bộ:

**An toàn**
1. Ngắt điện đột ngột giữa lúc chạy → mở lại app phát hiện được phiên dở dang và hoàn tác sạch 100%.
2. Undo trả file về đúng vị trí cũ với 100% file, timestamp giữ nguyên.
3. Không tồn tại bất kỳ đường dẫn mã nguồn nào gọi lệnh xoá vĩnh viễn.
4. Thư mục chứa file có đường dẫn 300 ký tự → xử lý được, không lỗi.
5. Thư mục chứa file tên `CON.txt`, `aux.pdf`, tên có emoji, tên tiếng Nhật → không lỗi.
6. Thư mục OneDrive có file placeholder → **không** kích hoạt tải về.
7. Thư mục có junction trỏ về chính nó → không treo, không lặp vô hạn.
8. File đang mở trong Word → bỏ qua an toàn, báo rõ, các file khác vẫn chạy tiếp.
9. Di chuyển từ ổ C sang ổ D bị ngắt giữa chừng → file nguồn còn nguyên vẹn.
10. Chạy trên `C:\Windows` → bị chặn.

**Đúng đắn**
11. Preview khớp 100% với kết quả thực tế sau khi Apply.
12. Không có file nào bị ghi đè trong bất kỳ kịch bản nào.
13. Bộ 1.000 file mẫu tiếng Việt có dấu → Auto-Project gom đúng ≥85% theo đánh giá thủ công.
14. Bật SIDECAR_BUNDLE → cặp `.CR2`/`.JPG` cùng tên luôn nằm chung thư mục.
15. Bật FOLDER_UNIT → thư mục có `.git` không bị phá cấu trúc.

**Hiệu năng**
16. Đạt toàn bộ chỉ tiêu §12 trên máy cấu hình trung bình (i5, 8GB RAM, SSD).

---

## PHỤ LỤC A — NHÓM LOẠI FILE MẶC ĐỊNH

| Nhóm | Đuôi file |
|---|---|
| `01-Hinh-Anh` | jpg jpeg png gif bmp webp heic heif tiff svg ico avif |
| `02-Anh-RAW` | cr2 cr3 nef arw dng raf orf rw2 pef srw |
| `03-Video` | mp4 mkv avi mov wmv flv webm m4v mpg mpeg 3gp ts |
| `04-Am-Thanh` | mp3 wav flac aac ogg wma m4a opus aiff |
| `05-Tai-Lieu` | pdf doc docx txt rtf odt md epub pages |
| `06-Bang-Tinh` | xls xlsx csv ods numbers tsv |
| `07-Trinh-Chieu` | ppt pptx odp key |
| `08-Nen` | zip rar 7z tar gz bz2 xz iso |
| `09-Cai-Dat` | exe msi msix appx bat cmd ps1 |
| `10-Thiet-Ke` | psd ai indd sketch fig xd afdesign cdr |
| `11-Lap-Trinh` | py js ts jsx tsx java c cpp cs go rs php rb sql json xml yaml html css |
| `12-3D-CAD` | dwg dxf stl obj fbx blend 3ds skp step |
| `13-Font` | ttf otf woff woff2 eot |
| `14-Khac` | (mọi thứ còn lại) |

Tiền tố số giúp thư mục tự sắp đúng thứ tự mong muốn trong Explorer — có thể tắt trong Cài đặt.

## PHỤ LỤC B — TỪ KHOÁ NGHIỆP VỤ MẶC ĐỊNH (KEYWORD_RULE)

| Thư mục | Từ khoá (không phân biệt dấu/hoa thường) |
|---|---|
| `Hop-Dong` | hop dong, contract, hd, agreement, thoa thuan |
| `Hoa-Don` | hoa don, invoice, vat, bill, receipt, bien lai |
| `Bao-Cao` | bao cao, report, tong ket, summary, thong ke |
| `Ho-So-Nhan-Su` | cv, resume, ho so, don xin, hop dong lao dong, bang cap |
| `Ke-Hoach` | ke hoach, plan, roadmap, chien luoc, proposal, de xuat |
| `Bao-Gia` | bao gia, quotation, quote, chao gia |
| `Van-Ban-Phap-Ly` | quyet dinh, thong tu, nghi dinh, cong van, giay phep |
| `Tai-Chinh` | ngan sach, budget, thu chi, cong no, bang luong |

Người dùng sửa/thêm được toàn bộ trong Cài đặt.
