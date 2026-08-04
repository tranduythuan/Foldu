# Foldu không gửi dữ liệu của bạn đi đâu cả

**Tiếng Việt** · [English](PRIVACY.en.md)

Đây là phần mềm miễn phí, mã nguồn mở, do một người làm. Bạn không có lý do gì để tin tôi chỉ vì tôi nói vậy. Nên tài liệu này không viết theo kiểu "chúng tôi cam kết" — mà viết theo kiểu **mọi câu ở đây bạn đều tự kiểm tra được**.

---

## Tóm tắt một câu

Foldu **không có khả năng kết nối mạng**. Không phải "tôi hứa sẽ không gửi dữ liệu", mà là **trong phần mềm không có bộ phận nào để gửi**.

---

## Đừng tin tôi — tự kiểm tra

Xếp từ dễ nhất đến chắc chắn nhất. Chỉ cần làm cách số 1 là đủ yên tâm cho hầu hết mọi người.

### 1. Chặn hẳn bằng Tường lửa Windows rồi dùng thử

Cách thuyết phục nhất, và ai cũng làm được:

1. Bấm Start, gõ **Windows Defender Firewall with Advanced Security**, mở lên
2. Chọn **Outbound Rules** → **New Rule…**
3. Chọn **Program** → Next → trỏ tới file `foldu.exe` của bạn
4. Chọn **Block the connection** → Next → Next → đặt tên gì cũng được → Finish

Giờ Windows cấm tuyệt đối Foldu ra Internet. Mở phần mềm lên dùng thử **toàn bộ tính năng**: quét, sắp xếp, tìm trùng lặp, xem ảnh gần giống, xếp ảnh theo nơi chụp, hoàn tác.

**Mọi thứ vẫn chạy đủ, không thiếu một chức năng nào.** Vì nó vốn chưa bao giờ cần mạng.

### 2. Xem trực tiếp lưu lượng mạng

Bấm `Windows + R`, gõ `resmon`, sang thẻ **Network**. Mở Foldu và chạy thử. `foldu.exe` sẽ **không bao giờ xuất hiện** trong danh sách tiến trình có hoạt động mạng.

### 3. Đọc danh sách thư viện — chỉ có 13 dòng

Toàn bộ thư viện Foldu dùng nằm trong [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml). Đây là danh sách đầy đủ, không giấu gì:

| Thư viện | Dùng để làm gì |
|---|---|
| `tauri` | Dựng cửa sổ ứng dụng (`features = []` — không bật thêm bất kỳ mở rộng nào) |
| `serde`, `serde_json` | Đọc/ghi file cấu hình |
| `rayon` | Chia việc ra nhiều nhân CPU cho nhanh |
| `blake3` | Băm nội dung file để tìm bản trùng nhau |
| `chrono` | Xử lý ngày giờ |
| `trash` | Đưa file vào Thùng rác Windows |
| `kamadak-exif` | Đọc ngày chụp, máy ảnh, toạ độ GPS trong ảnh |
| `imagesize`, `image` | Đọc kích thước và giải mã ảnh |
| `unicode-normalization` | Bỏ dấu tiếng Việt khi đặt tên thư mục |
| `once_cell` | Chuyện kỹ thuật vặt trong Rust |
| `rfd` | Hộp thoại chọn thư mục của Windows |

**Không có một thư viện mạng nào** — không `reqwest`, không `hyper`, không `ureq`, không gì cả. Một chương trình Rust muốn gửi dữ liệu đi thì phải có thư viện như vậy, hoặc tự viết mã socket. Bạn có thể tìm cả hai trong mã nguồn và sẽ không thấy.

### 4. Đọc mã nguồn

Toàn bộ mã nguồn công khai theo giấy phép MIT tại repo này. Không có phần nào bị giấu, không có thư viện đóng.

### 5. Tự build lấy mà dùng

Cách chắc chắn tuyệt đối: làm theo mục "Build từ mã nguồn" trong [README](README.md#build-từ-mã-nguồn). File `.exe` bạn tự tạo ra từ mã nguồn bạn vừa đọc.

---

## Vậy sao mấy tính năng "thông minh" vẫn chạy được?

Đây là câu hỏi hay nhất, vì nhiều người tưởng nhận diện ảnh thì phải có AI hoặc máy chủ. Không cần. **Hiểu biết nằm trong phép tính, không nằm trong dữ liệu phải tải về.**

### Tìm ảnh gần giống nhau

Foldu thu mỗi tấm ảnh xuống còn **9×8 = 72 chấm xám**, rồi đi từng hàng hỏi *"chấm này có sáng hơn chấm bên phải không?"*. 64 câu trả lời có/không đó là "vân tay" của tấm ảnh.

Điểm mấu chốt: nó ghi **quan hệ giữa hai chấm cạnh nhau**, không ghi độ sáng tuyệt đối. Nên ảnh bị thu nhỏ, nén lại, sáng lên hay tối đi thì quan hệ đó phần lớn vẫn giữ nguyên. So hai ảnh chỉ là đếm xem 64 câu trả lời lệch nhau bao nhiêu chỗ — chính là con số **"lệch N bit"** hiện trong phần mềm.

Ví von: giống như bạn nhận ra hai bản thu của cùng một bài hát bằng cách ngân nga giai điệu rồi so. Không cần thư viện nhạc nào, giai điệu nằm ngay trong bản nhạc.

Mã nguồn: [`src-tauri/src/phash.rs`](src-tauri/src/phash.rs)

### Xếp ảnh theo nơi chụp

Điện thoại ghi sẵn toạ độ GPS vào ảnh. Foldu tra toạ độ ra tên thành phố bằng **bảng ~34.000 thành phố toàn thế giới nhúng thẳng vào file `.exe`** (khoảng 730KB, dữ liệu từ [GeoNames](https://www.geonames.org), giấy phép CC BY 4.0). Không gọi dịch vụ bản đồ nào.

Mã nguồn: [`src-tauri/src/geo.rs`](src-tauri/src/geo.rs)

### Tìm file trùng nhau

Băm nội dung file bằng BLAKE3 rồi so với nhau. Thuần tính toán trên máy bạn.

### Không có AI, không có mô hình tải về

Foldu **không** nhận diện nội dung ảnh kiểu *"tấm này có con mèo"*. Cái đó mới cần mô hình đã huấn luyện. Đó là ranh giới tôi cố ý không bước qua, vì nó sẽ phá vỡ đúng lời hứa này.

---

## Foldu lưu gì trên máy bạn

Tất cả nằm trong **một thư mục duy nhất**: `%APPDATA%\Foldu\`
(dán `%APPDATA%\Foldu` vào thanh địa chỉ File Explorer là mở được)

| Chỗ | Chứa gì | Vì sao cần |
|---|---|---|
| `settings.json` | Nhóm loại file, từ khoá, thư mục mở gần đây, ngôn ngữ, nền sáng/tối | Nhớ cài đặt của bạn |
| `journal\*.jsonl` | **Đường dẫn đầy đủ của từng file đã chuyển** (từ đâu → tới đâu) | Bắt buộc phải có thì mới hoàn tác được |
| `profiles\` | Hồ sơ cấu hình bạn tự lưu | Chỉ có khi bạn tự xuất ra |
| `reports\` | Báo cáo bạn bấm "Lưu báo cáo" | Chỉ có khi bạn tự bấm |

**Nói rõ:** nhật ký và báo cáo **có chứa tên và đường dẫn đầy đủ** của file trong thư mục bạn dọn. Không có cách nào khác — muốn trả file về đúng chỗ cũ thì phải nhớ chỗ cũ ở đâu. Chúng nằm trên máy bạn và không đi đâu cả.

**Xoá sạch dấu vết:** xoá thư mục `%APPDATA%\Foldu\` là hết. Với bản chạy thẳng, gỡ phần mềm chỉ là xoá file `.exe` và thư mục đó (xem thêm mục registry bên dưới).

---

## Thứ duy nhất có thể lộ ra ngoài là do chính bạn

Foldu không gửi gì, nhưng có một file **bạn có thể vô tình chia sẻ**:

**Báo cáo xuất ra (HTML/CSV)** chứa **tên và đường dẫn đầy đủ** của mọi file được xử lý. Nếu bạn gửi báo cáo đó cho người khác, thì bạn đang cho họ xem cấu trúc thư mục và tên file của mình. Hãy cân nhắc trước khi gửi đi.

Ngược lại, có hai thứ tôi cố tình chặn:

- **Ảnh thu nhỏ không bao giờ được ghi ra đĩa.** Chỉ nằm trong bộ nhớ, tắt phần mềm là hết. (Windows Explorer thì *đã* lưu sẵn ảnh thu nhỏ của bạn vào `thumbcache` từ lâu — Foldu để lại ít dấu vết hơn.)
- **Ảnh không bao giờ được nhúng vào báo cáo**, đúng vì báo cáo là file có thể bị gửi đi. Báo cáo chỉ có chữ.

---

## Foldu không làm những việc sau

- ❌ Không có tài khoản, không đăng nhập, không kích hoạt
- ❌ Không thu thập thống kê sử dụng (telemetry)
- ❌ Không gửi báo cáo lỗi tự động
- ❌ Không tự kiểm tra cập nhật (muốn bản mới thì bạn tự vào trang Releases tải)
- ❌ Không quảng cáo, không bản Pro, không mời nâng cấp
- ❌ Không đọc file ngoài thư mục bạn chọn
- ❌ Không cần quyền quản trị (Administrator)

**Về registry, nói cho chính xác:** bản chạy thẳng `foldu.exe` **không ghi gì vào registry**. Còn nếu bạn dùng **bộ cài** thì nó ghi một mục gỡ cài đặt (trong `HKEY_CURRENT_USER`, không phải toàn máy) — đúng như mọi phần mềm Windows khác, để Foldu hiện ra trong danh sách "Apps & features" cho bạn gỡ. Gỡ bằng trình gỡ cài đặt là mục đó biến mất theo. Muốn tuyệt đối không đụng registry thì dùng bản `foldu.exe` chạy thẳng.

---

## Nó xin những quyền gì trên máy

Đúng ba thứ, đều là thứ tối thiểu để làm việc của nó:

1. **Đọc và ghi file trong thư mục bạn chọn** — để sắp xếp file
2. **Thùng rác Windows** — chỗ duy nhất file thừa được đưa tới; Foldu không bao giờ xoá vĩnh viễn
3. **Hộp thoại chọn thư mục** — để bạn trỏ vào thư mục cần dọn

Ngoài ra, có một chi tiết kỹ thuật đáng nói cho người kỹ tính: Foldu **không bật giao thức `asset:`** của Tauri (giao thức cho phép lớp giao diện đọc thẳng file từ ổ đĩa). Thư mục `capabilities/` không tồn tại trong dự án — bạn kiểm tra được. Ảnh thu nhỏ được lõi xử lý rồi truyền sang giao diện dưới dạng dữ liệu nhúng, nên lớp giao diện không hề có quyền đọc ổ đĩa.

---

## Một chi tiết tôi nói thẳng cho công bằng

Foldu dùng **WebView2** để vẽ giao diện — đây là thành phần có sẵn của Windows (đi kèm Microsoft Edge), không phải của tôi. Foldu chỉ nạp file HTML/CSS/JS nằm trong chính nó vào đó, và đặt chính sách bảo mật `default-src 'self'` để trang không thể tải bất cứ thứ gì từ ngoài về.

Tuy nhiên **WebView2 là phần mềm của Microsoft và có cơ chế cập nhật riêng của nó, cùng với Windows/Edge** — chuyện đó nằm ngoài tầm kiểm soát của Foldu và của tôi. Nếu bạn chặn `foldu.exe` bằng tường lửa như hướng dẫn ở trên thì tiến trình Foldu không ra ngoài được, đó là điều tôi có thể đảm bảo.

---

## Cảnh báo SmartScreen không phải là virus

Lần đầu mở, Windows có thể hiện bảng xanh *"Windows protected your PC"*. Lý do: phần mềm miễn phí này **chưa mua chứng chỉ ký số** (khoảng vài trăm đô một năm). Windows cảnh báo với mọi phần mềm chưa ký, bất kể sạch hay bẩn.

Bấm **More info → Run anyway** là chạy được.

Muốn chắc chắn file tải về đúng là file tôi phát hành, mỗi bản trên trang [Releases](https://github.com/tranduythuan/Foldu/releases) đều kèm mã kiểm tra **SHA-256**. Mở PowerShell và chạy:

```bash
Get-FileHash foldu.exe -Algorithm SHA256
```

So chuỗi kết quả với chuỗi ghi trên trang Releases. Khớp nghĩa là file nguyên vẹn, không bị ai sửa.

---

## Nếu bạn vẫn còn nghi ngờ

Hoàn toàn hợp lý. Cách chắc chắn nhất, không cần tin ai:

**Chặn `foldu.exe` bằng tường lửa** (mục 1 ở trên) và **tự build từ mã nguồn** (mục 5). Sau đó bạn không cần tin tôi nữa — bạn có bằng chứng.

Thấy điều gì trong tài liệu này không khớp với mã nguồn, xin mở một [issue](https://github.com/tranduythuan/Foldu/issues). Tôi sẽ sửa.

---

*Tài liệu này thuộc dự án [Foldu](README.md) — Trần Duy Thuận, <https://tranduythuan.com>. Giấy phép MIT.*
