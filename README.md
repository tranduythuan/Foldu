# Foldu

**Tiếng Việt** · [English](README.en.md)

Phần mềm desktop sắp xếp và dọn dẹp file tự động — an toàn, hoàn tác được — viết bằng
**Rust + Tauri 2**, giao diện HTML/CSS. Chạy offline hoàn toàn, một file `.exe` duy nhất.

Đây là bản hiện thực của [Design_Specification_v2.md](Design_Specification_v2.md) — Giai đoạn 1 (MVP).

---

## Tải về (cho người dùng — không cần biết lập trình)

Vào trang **[Releases](https://github.com/tranduythuan/Foldu/releases/latest)** rồi tải một trong hai:

- **`foldu.exe`** — bản chạy thẳng, **không cần cài**. Tải về, bấm đúp là chạy.
- **`Foldu_1.0.0_x64-setup.exe`** — bản cài đặt, tạo lối tắt ở Start Menu, gỡ như phần mềm bình thường.

Chạy trên **Windows 10 / 11**. Máy cần **WebView2** (có sẵn trên gần như mọi máy Windows đã cập nhật; nếu thiếu, Windows tự tải về trong ~30 giây).

> **Lần đầu mở, Windows có thể hiện bảng xanh "Windows protected your PC".** Đó là vì phần mềm miễn phí này chưa mua chứng chỉ ký số, **không phải virus**. Bấm **More info → Run anyway** là chạy được. (Muốn hết cảnh báo hẳn thì phải mua chứng chỉ ký số ~vài trăm USD/năm — không cần thiết cho bản cá nhân.)

🔒 **Foldu không gửi dữ liệu của bạn đi đâu cả — và bạn tự kiểm tra được điều đó.**
Xem [PRIVACY.md](PRIVACY.md): cách chặn phần mềm bằng tường lửa rồi thấy nó vẫn chạy đủ tính năng, vì sao nhận diện ảnh gần giống không cần mạng, và Foldu lưu đúng những gì trên máy bạn.

---

## Chạy và chia sẻ (khi tự build)

Sau khi build, file cần chia sẻ nằm ở:

```
src-tauri/target/release/foldu.exe
```

Đó là **một file duy nhất**, khoảng 6–10 MB. Copy sang máy nào cũng chạy được, không cần cài đặt gì.

**Yêu cầu duy nhất trên máy người nhận:** WebView2 Runtime — có sẵn trên mọi bản Windows 11 và
gần như mọi bản Windows 10 đã cập nhật (đi kèm Microsoft Edge). Nếu máy nào thiếu, Windows sẽ báo
và tải về trong khoảng 30 giây, hoặc dùng bản `.msi` trong `target/release/bundle/nsis/`.

---

## Build từ mã nguồn

Cần: Rust (rustup), MSVC Build Tools + Windows SDK, Node.js.

```bash
npm install
npm run build
```

Chạy thử ở chế độ phát triển (có hot reload cho giao diện):

```bash
npm run dev
```

Chạy toàn bộ test:

```bash
cd src-tauri && cargo test
```

---

## Hai ngôn ngữ

Tiếng Việt và tiếng Anh, đổi bằng nút cờ ở góc dưới trái. Cờ vẽ bằng SVG chứ không
dùng emoji, vì Windows không có glyph cờ nên emoji cờ hiện ra thành hai chữ cái.

Điều quan trọng: đổi ngôn ngữ **đổi luôn tên thư mục phần mềm tạo ra trên ổ đĩa**.
`01-Hinh-Anh` thành `01-Images`, `02-7-Ngay-Qua` thành `02-Past-7-Days`, `Anh-Chup-Man-Hinh`
thành `Screenshots`. Nếu chỉ dịch chữ trên màn hình mà để tên thư mục nguyên tiếng Việt
thì người nước ngoài mở ổ đĩa ra vẫn không hiểu gì.

Có một test bắt buộc mọi tên thư mục sinh ra, ở cả hai ngôn ngữ, phải là ASCII thuần,
không chứa ký tự Windows cấm và không kết thúc bằng dấu chấm hay khoảng trắng.

Bảng chữ nằm ở hai chỗ: `src-tauri/src/i18n.rs` cho lõi (tên thư mục, thông báo hệ thống,
danh mục cách chia, mẫu dựng sẵn) và `ui/i18n.js` cho giao diện. Thêm ngôn ngữ thứ ba
là thêm một cột vào hai bảng đó.

Khi đổi ngôn ngữ, nếu bảng nhóm file và bảng từ khoá vẫn đúng y bộ mặc định của ngôn ngữ
cũ thì chúng tự đổi sang bộ mặc định của ngôn ngữ mới. Ai đã sửa tay thì giữ nguyên,
vì đó là dữ liệu của họ.

## Kiến trúc

Nguyên tắc quan trọng nhất: **`planner` tuyệt đối không ghi vào ổ đĩa.** Nó chỉ nhận danh sách file
và trả về một `Plan`. Chỉ `executor` mới được phép ghi. Nhờ vậy bản Xem trước và lúc Áp dụng dùng
chung một mã nguồn — cái người dùng nhìn thấy chính xác là cái sẽ xảy ra.

```
src-tauri/src/
├── main.rs         Điểm khởi động
├── lib.rs          Lớp lệnh Tauri, nối giao diện với lõi
├── util.rs         Chuẩn hoá tiếng Việt, làm sạch tên thư mục, định dạng
├── config.rs       Nhóm loại file, từ khoá, hồ sơ, mẫu dựng sẵn
├── safety.rs       Thư mục cấm, preflight, dò file bị khoá, dung lượng trống
├── scanner.rs      Duyệt cây thư mục, bộ lọc, thuộc tính Windows
├── media.rs        EXIF, kích thước ảnh, magic bytes, Zone.Identifier
├── clustering.rs   AUTO_PROJECT v2 — nhận diện cụm dự án
├── criteria.rs     24 tiêu chí sắp xếp
├── dedup.rs        Lọc trùng lặp 3 tầng (BLAKE3)
├── planner.rs      Lập kế hoạch — KHÔNG ghi ổ đĩa
├── journal.rs      Write-Ahead Journal (JSONL)
├── executor.rs     Thực thi + hoàn tác — lớp DUY NHẤT ghi ổ đĩa
└── analytics.rs    Bảng phân tích, điểm sức khoẻ thư mục

ui/                 Giao diện (HTML + CSS + JS thuần, không build step)
```

---

## Những gì đã có

**24 tiêu chí sắp xếp**, xếp tối đa 4 tầng, kèm 8 mẫu dựng sẵn:

| Nhóm | Tiêu chí |
|---|---|
| Cơ bản | `TYPE` `EXT` `REAL_TYPE` `SIZE_TIER` `SIZE_TIER_AUTO` `ALPHABET` |
| Thời gian | `TIME_MODIFIED` `TIME_CREATED` `TIME_TAKEN` `TIME_RELATIVE` `TIME_QUARTER` `TIME_WEEK` `ACCESS_HEAT` |
| Nội dung | `MEDIA_RESOLUTION` `IMAGE_ORIENTATION` `EXIF_CAMERA` `SCREENSHOT_DETECT` |
| Ngữ nghĩa | `AUTO_PROJECT` `VERSION_GROUP` `KEYWORD_RULE` `DOWNLOAD_SOURCE` `LANGUAGE_SCRIPT` |
| Hệ thống | `ORIGIN_FOLDER` `LITERAL` |

**An toàn dữ liệu:**

- **Write-Ahead Journal** — toàn bộ kế hoạch được ghi và `fsync` **trước** thao tác đầu tiên.
  Mất điện giữa chừng vẫn hoàn tác được; mở lại app sẽ hiện banner cảnh báo.
- Cùng ổ đĩa dùng `rename` (nguyên tử); khác ổ đĩa thì **copy → đối chiếu hash BLAKE3 → mới xoá nguồn**.
- Không bao giờ ghi đè: trùng tên thì thêm hậu tố `(1)`, `(2)` — bằng vòng lặp, không đệ quy.
- Không bao giờ xoá vĩnh viễn: mọi thao tác xoá đều vào Thùng rác Windows.
- Chặn cứng thư mục hệ thống, gốc ổ đĩa, `AppData`.
- Làm sạch tên thư mục: ký tự cấm, tên thiết bị (`CON`, `AUX`…), đuôi dấu chấm/khoảng trắng.
- Bỏ qua symlink/junction (chống đệ quy vô hạn) và **file đám mây chưa tải về**
  (đọc `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` — tránh kích hoạt tải hàng trăm GB).
- Preflight: quyền ghi, file đang bị ứng dụng khác khoá, dung lượng trống, đường dẫn quá dài.
- Giữ nguyên dấu thời gian sau khi di chuyển.
- Hoàn tác chỉ dọn thư mục rỗng **do chính phần mềm tạo ra**, không đụng thư mục có sẵn.

**Dọn cả ổ đĩa / phân vùng:**

Màn Bắt đầu liệt kê mọi ổ đang gắn kèm thanh dung lượng; bấm một ổ là chọn cả phân vùng đó.
Bốn lớp bảo vệ chạy đồng thời:

1. **Ổ chứa Windows bị chặn tuyệt đối** ở cấp gốc — không có tuỳ chọn nào mở được. Muốn dọn
   trong ổ hệ thống thì phải chọn thư mục con cụ thể.
2. **Mục hệ thống ở gốc ổ được bảo vệ vô điều kiện** — `$RECYCLE.BIN`, `System Volume Information`,
   `Config.Msi`, `pagefile.sys`, `hiberfil.sys`, `$WinREAgent`, `Recovery`, và cả `Windows` /
   `Program Files` / `Users` phòng trường hợp ổ từng chứa một bản Windows khác. Danh sách này
   **không phụ thuộc** vào tuỳ chọn "hiện file ẩn / hệ thống" của người dùng.
3. **Thư mục ứng dụng được để nguyên tại chỗ** — nhận diện qua trình gỡ cài đặt (`unins*.exe`),
   thư viện Steam, cấu trúc Electron/Chromium, hoặc `.exe` đi kèm từ 3 thư viện `.dll` trở lên.
   Di chuyển những thư mục này sẽ làm hỏng đường dẫn trong registry và shortcut.
4. **Junction / symlink không đi theo** — `D:\OneDrive` kiểu reparse point được bỏ qua, tránh
   vòng lặp vô hạn và tránh kích hoạt tải dữ liệu đám mây.

Màn Phân tích khi quét nguyên ổ hiển thị thêm: thanh dung lượng chia ba phần (trong phạm vi sắp xếp
/ được bảo vệ / còn trống) và bảng **thư mục cấp 1 chiếm nhiều nhất** — nhìn một cái là biết cái gì
đang ăn hết ổ.

**Xếp ảnh theo nơi chụp** — đọc toạ độ GPS ghi sẵn trong ảnh (EXIF) rồi tra ra tên
thành phố hoàn toàn offline, tạo thư mục `Da-Nang`, `Tokyo`, `Munich`... Bảng **~34.000
thành phố toàn thế giới** (dữ liệu GeoNames, dân số trên 15.000) nhúng thẳng trong file
chạy dưới dạng nhị phân gọn (~730KB), tra bằng lưới ô 1° nên nhanh dù nhiều điểm.

Hai chốt để ra tên đúng ý người dùng:
- **Ưu tiên thành phố lớn nhất trong 30km**, không lấy điểm gần nhất máy móc. Nếu không,
  ảnh giữa Hà Nội sẽ ra tên một phường (`Yen-Phu`) thay vì `Hanoi`, vì phường ở sát hơn.
- **Chốt an toàn 150km**: xa hơn thì không đoán bừa mà vào `Khong-Ro-Noi-Chup`.

Chỉ ảnh chụp bằng điện thoại có bật định vị mới có GPS. Dữ liệu thành phố từ
[GeoNames](https://www.geonames.org), giấy phép CC BY 4.0.

**Đổi tên hàng loạt** — lắp mẫu tên từ các mảnh (ngày, tên gốc, tên thư mục, số thứ tự,
chữ tự gõ) kèm dọn dẹp (bỏ dấu, chữ thường, gạch nối). Có bản xem trước sống trong lúc lắp.

Vì đổi tên chính là chuyển file trong cùng thư mục, nó tái dùng nguyên bộ máy nhật ký +
hoàn tác, nên **bấm một nút là mọi tên gốc trở lại đúng từng cái**, kể cả sau khi crash.
Ba lớp an toàn cứng: đuôi file (.jpg, .pdf) luôn được giữ, người dùng không đổi được; tên
rỗng thì lùi về tên gốc; ký tự cấm Windows và tên thiết bị cấm được làm sạch. Hai file ra
cùng tên thì thêm số, không bao giờ đè lên nhau. Số thứ tự đánh theo từng thư mục, ổn định
nên bản xem trước khớp lúc chạy thật.

**Ràng buộc thông minh:**

- `cleanEmptyDirs` — sau khi lôi file ra khỏi các thư mục con, các vỏ thư mục rỗng còn lại
  được dọn theo (duyệt hậu thứ tự, từ trong ra ngoài). Không bao giờ đụng vào chính thư mục
  người dùng chọn, không đụng mục hệ thống, không đi theo lối tắt. Mỗi thư mục bị dọn đều ghi
  vào nhật ký để Hoàn tác dựng lại đúng cấu trúc cũ. Chỉ chạy ở chế độ Chuyển đi.
- `keepSidecarTogether` — RAW+JPG, mp4+srt, psd+preview luôn nằm chung thư mục.
- `treatProjectFoldersAsUnit` — thư mục có `.git` / `package.json` / `Cargo.toml` được di chuyển
  nguyên khối, không bị phá tung.
- `protectInstalledApps` — thư mục ứng dụng đã cài để nguyên tại chỗ.

**Trùng lặp** — lọc 3 tầng: kích thước → hash nhanh 8KB đầu+cuối → BLAKE3 toàn file (đa luồng).
4 chiến lược chọn bản giữ lại, 3 cách xử lý bản thừa.

**Ảnh gần giống** — băm tri giác dHash 64 bit: thu ảnh về 9×8 mức xám rồi so từng cặp
điểm cạnh nhau. Bắt được cùng một tấm ảnh lưu nhiều lần ở kích thước hoặc mức nén khác
nhau, thứ mà hash byte hoàn toàn chịu. Đo trên ảnh thật: bản 1200×900 và bản 400×300 nén
lại JPEG cho ra **lệch 0 bit**, ảnh khác hẳn lệch 36 bit.

Gom nhóm bằng hợp-tìm trên các cặp dưới ngưỡng, kèm chặn theo tỷ lệ khung để ảnh ngang
không bao giờ ghép với ảnh dọc. Bản giữ lại là ảnh nhiều điểm ảnh nhất.

Ràng buộc an toàn: đây là **phỏng đoán**, không phải giống hệt. Phần thừa luôn được dồn
vào thư mục riêng `_Anh-Gan-Giong` để người dùng tự xem lại, **không bao giờ** vào Thùng
rác kể cả khi người dùng đặt thế cho trùng lặp tuyệt đối. Nhãn trong danh sách cũng dùng
màu khác hẳn.

**Chế độ thao tác:** `MOVE` · `COPY` · `HARDLINK` (cấu trúc mới không tốn thêm byte nào) · `REPORT_ONLY`.

---

## Chưa có (Giai đoạn 2–4 của bản đặc tả)

Rules Engine (if/then) · Watcher & Scheduler · Chính sách lưu trữ · CLI · `AI_SEMANTIC`.

---

## Nơi lưu dữ liệu

```
%APPDATA%\Foldu\
├── settings.json     Nhóm loại file, từ khoá, từ nhiễu, thư mục gần đây
├── journal\          Nhật ký từng phiên (.jsonl) — dùng để hoàn tác
├── profiles\         Hồ sơ cấu hình xuất ra để chia sẻ
└── reports\          Báo cáo HTML / CSV
```

Gỡ bản chạy thẳng = xoá file `.exe` và thư mục trên, không để lại gì trong registry. Bản cài đặt thì gỡ qua "Apps & features" như phần mềm bình thường. Chi tiết đầy đủ về dữ liệu và quyền riêng tư: [PRIVACY.md](PRIVACY.md).

---

## Khác biệt so với bản đặc tả

| Đặc tả | Thực tế | Lý do |
|---|---|---|
| SQLite (`rusqlite`) cho nhật ký | JSONL ghi nối tiếp | Ghi nối tiếp an toàn hơn khi crash (một dòng hỏng không hỏng cả file), không cần biên dịch C, khởi động nhanh hơn |
| React + Tailwind + shadcn/ui | HTML/CSS/JS thuần | Không cần build step, dựng được đúng thẩm mỹ mong muốn, giảm rủi ro chuỗi công cụ |
| `jwalk` | Bộ duyệt tự viết | Cần dừng sớm ở thư mục dự án và kiểm soát reparse point — logic riêng dễ hơn là gò theo thư viện |

---

## Tác giả & giấy phép

**Foldu** — một sản phẩm cá nhân, mã nguồn mở, miễn phí cho tất cả mọi người.

- **Tác giả / chủ dự án:** Trần Duy Thuận — <https://tranduythuan.com>
- **Viết mã:** Claude (Anthropic)
- **Giấy phép:** [MIT](LICENSE) — © 2026 Trần Duy Thuận. Bạn được tự do dùng, sửa, chia sẻ.

Dữ liệu thành phố dùng cho tính năng xếp ảnh theo nơi chụp lấy từ
[GeoNames](https://www.geonames.org), giấy phép **CC BY 4.0**.
