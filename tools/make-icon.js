// Sinh anh nguon cho bo icon (1024x1024 PNG), khong dung thu vien ngoai.
// Chay: node tools/make-icon.js  ->  assets/icon.png
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const S = 1024;
const RADIUS = 224;

// Bang mau
const BG_TOP = [99, 102, 241];   // indigo-500
const BG_BOT = [55, 48, 163];    // indigo-800
const BAR = [255, 255, 255];
const ACCENT = [165, 243, 252];  // cyan-200

function lerp(a, b, t) { return a + (b - a) * t; }

/** Khoang cach am/duong toi hinh chu nhat bo goc (dung de khu rang cua) */
function roundRectSDF(x, y, w, h, r) {
  const qx = Math.abs(x) - (w / 2 - r);
  const qy = Math.abs(y) - (h / 2 - r);
  const ax = Math.max(qx, 0), ay = Math.max(qy, 0);
  return Math.hypot(ax, ay) + Math.min(Math.max(qx, qy), 0) - r;
}

/** Do phu (0..1) voi khu rang cua 1px */
function cover(sdf) {
  return Math.min(1, Math.max(0, 0.5 - sdf));
}

const px = Buffer.alloc(S * S * 4);

// Ba thanh ngang thu ngan dan - doc ra ngay la "sap xep"
const bars = [
  { w: 560, y: 372 },
  { w: 420, y: 512 },
  { w: 280, y: 652 },
];
const BAR_H = 88;
const BAR_R = 44;
const BAR_LEFT = 232;

for (let y = 0; y < S; y++) {
  for (let x = 0; x < S; x++) {
    const i = (y * S + x) * 4;

    // Nen: hinh vuong bo goc, chuyen mau doc
    const bgCover = cover(roundRectSDF(x - S / 2 + 0.5, y - S / 2 + 0.5, S - 24, S - 24, RADIUS));
    const t = y / S;
    let r = lerp(BG_TOP[0], BG_BOT[0], t);
    let g = lerp(BG_TOP[1], BG_BOT[1], t);
    let b = lerp(BG_TOP[2], BG_BOT[2], t);

    // Anh sang nhe o goc tren trai cho co chieu sau
    const glow = Math.max(0, 1 - Math.hypot(x - 300, y - 240) / 720);
    r = Math.min(255, r + glow * 46);
    g = Math.min(255, g + glow * 46);
    b = Math.min(255, b + glow * 52);

    // Ba thanh
    let barCover = 0;
    for (const bar of bars) {
      const cx = BAR_LEFT + bar.w / 2;
      const c = cover(roundRectSDF(x - cx + 0.5, y - bar.y + 0.5, bar.w, BAR_H, BAR_R));
      barCover = Math.max(barCover, c);
    }
    if (barCover > 0) {
      r = lerp(r, BAR[0], barCover);
      g = lerp(g, BAR[1], barCover);
      b = lerp(b, BAR[2], barCover);
    }

    // Cham nhan mau cyan o cuoi thanh ngan nhat
    const dot = cover(Math.hypot(x - 640, y - 652) - 44);
    if (dot > 0) {
      r = lerp(r, ACCENT[0], dot);
      g = lerp(g, ACCENT[1], dot);
      b = lerp(b, ACCENT[2], dot);
    }

    px[i] = Math.round(r);
    px[i + 1] = Math.round(g);
    px[i + 2] = Math.round(b);
    px[i + 3] = Math.round(bgCover * 255);
  }
}

// ---- Dong goi PNG (RGBA, khong loc)
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body) >>> 0);
  return Buffer.concat([len, body, crc]);
}

let CRC_TABLE = null;
function crc32(buf) {
  if (!CRC_TABLE) {
    CRC_TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c;
    }
  }
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return c ^ -1;
}

const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0; // filter type 0
  px.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8;   // bit depth
ihdr[9] = 6;   // color type RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);

const out = path.join(__dirname, '..', 'assets', 'icon.png');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, png);
console.log('Da tao ' + out + ' (' + png.length + ' bytes)');
