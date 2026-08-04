# Foldu

[Tiếng Việt](README.md) · **English**

A desktop app that sorts and tidies your files automatically — safely, and fully undoable.
Built with **Rust + Tauri 2**, HTML/CSS front end. Runs 100% offline as a single `.exe`.

This is the implementation of [Design_Specification_v2.md](Design_Specification_v2.md) — Phase 1 (MVP).

---

## Download (for users — no coding required)

Go to the **[Releases](https://github.com/tranduythuan/Foldu/releases/latest)** page and grab one of:

- **`foldu.exe`** — portable build, **no installation**. Download, double-click, done.
- **`Foldu_1.0.0_x64-setup.exe`** — installer, adds a Start Menu shortcut, uninstalls like any normal app.

Runs on **Windows 10 / 11**. Needs **WebView2** (already present on almost every up-to-date Windows machine; if missing, Windows fetches it in ~30 seconds).

> **On first launch, Windows may show a blue "Windows protected your PC" box.** That is because this free app isn't code-signed — it is **not a virus**. Click **More info → Run anyway**. (Removing the warning entirely would require a code-signing certificate at ~a few hundred USD/year — unnecessary for a personal project.)

🔒 **Foldu sends none of your data anywhere — and you can verify that yourself.**
See [PRIVACY.en.md](PRIVACY.en.md): how to block the app in your firewall and watch every feature still work, why near-duplicate photo matching needs no network, and exactly what Foldu stores on your machine.

---

## Build from source

Requires: Rust (rustup), MSVC Build Tools + Windows SDK, Node.js.

```bash
npm install
npm run build
```

Run in development mode (with UI hot reload):

```bash
npm run dev
```

Run the full test suite:

```bash
cd src-tauri && cargo test
```

---

## Two languages

Vietnamese and English, switched with the flag button at the bottom-left. Flags are drawn as
SVG rather than emoji, because Windows has no flag glyphs — flag emoji would show up as two letters.

The important part: switching language **also changes the folder names written to disk**.
`01-Hinh-Anh` becomes `01-Images`, `02-7-Ngay-Qua` becomes `02-Past-7-Days`, `Anh-Chup-Man-Hinh`
becomes `Screenshots`. If only the on-screen text were translated while folder names stayed in
Vietnamese, a foreign user opening the drive would still be lost.

There is a mandatory test asserting that every generated folder name, in both languages, is pure
ASCII, contains no Windows-forbidden characters, and never ends with a dot or a space.

The string tables live in two places: `src-tauri/src/i18n.rs` for the core (folder names, system
messages, the criteria catalog, presets) and `ui/i18n.js` for the interface. Adding a third
language means adding one column to those two tables.

When you switch language, if the file-type groups and keyword tables still match the previous
language's defaults, they switch to the new language's defaults. Anything the user has edited by
hand is left untouched — that's their data.

## Architecture

The single most important rule: **`planner` never writes to disk.** It only takes a list of files
and returns a `Plan`. Only `executor` is allowed to write. Because of this, the Preview and the
Apply steps share the same code — what the user sees is exactly what will happen.

```
src-tauri/src/
├── main.rs         Entry point
├── lib.rs          Tauri command layer, bridges the UI to the core
├── util.rs         Vietnamese normalization, folder-name sanitizing, formatting
├── config.rs       File-type groups, keywords, profiles, presets
├── safety.rs       Forbidden folders, preflight, locked-file detection, free space
├── scanner.rs      Directory-tree walk, filters, Windows attributes
├── media.rs        EXIF, image dimensions, magic bytes, Zone.Identifier
├── clustering.rs   AUTO_PROJECT v2 — project-cluster detection
├── criteria.rs     24 sorting criteria
├── dedup.rs        3-tier duplicate detection (BLAKE3)
├── planner.rs      Planning — does NOT write to disk
├── journal.rs      Write-Ahead Journal (JSONL)
├── executor.rs     Execute + undo — the ONLY layer that writes to disk
└── analytics.rs    Analytics dashboard, folder health score

ui/                 Interface (plain HTML + CSS + JS, no build step)
```

---

## What's implemented

**24 sorting criteria**, up to 4 nested levels, plus 8 built-in presets:

| Group | Criteria |
|---|---|
| Basic | `TYPE` `EXT` `REAL_TYPE` `SIZE_TIER` `SIZE_TIER_AUTO` `ALPHABET` |
| Time | `TIME_MODIFIED` `TIME_CREATED` `TIME_TAKEN` `TIME_RELATIVE` `TIME_QUARTER` `TIME_WEEK` `ACCESS_HEAT` |
| Content | `MEDIA_RESOLUTION` `IMAGE_ORIENTATION` `EXIF_CAMERA` `SCREENSHOT_DETECT` |
| Semantic | `AUTO_PROJECT` `VERSION_GROUP` `KEYWORD_RULE` `DOWNLOAD_SOURCE` `LANGUAGE_SCRIPT` |
| System | `ORIGIN_FOLDER` `LITERAL` |

**Data safety:**

- **Write-Ahead Journal** — the entire plan is written and `fsync`-ed **before** the first operation.
  A power cut mid-run is still undoable; reopening the app shows a warning banner.
- Same drive uses `rename` (atomic); across drives it's **copy → verify BLAKE3 hash → then delete source**.
- Never overwrites: on a name clash it appends a `(1)`, `(2)` suffix — via a loop, not recursion.
- Never deletes permanently: every delete goes to the Windows Recycle Bin.
- Hard-blocks system folders, drive roots, `AppData`.
- Sanitizes folder names: forbidden characters, device names (`CON`, `AUX`…), trailing dots/spaces.
- Skips symlinks/junctions (prevents infinite recursion) and **cloud files not yet downloaded**
  (reads `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` — avoids triggering hundreds of GB of downloads).
- Preflight: write permission, files locked by other apps, free space, path-too-long.
- Preserves timestamps after a move.
- Undo only removes empty folders **the app itself created**, never pre-existing ones.

**Clean a whole drive / partition:**

The Start screen lists every mounted drive with a usage bar; clicking a drive selects the whole
partition. Four protection layers run at once:

1. **The Windows drive is absolutely blocked** at the root — no option can unlock it. To tidy inside
   the system drive you must pick a specific subfolder.
2. **System items at the drive root are protected unconditionally** — `$RECYCLE.BIN`,
   `System Volume Information`, `Config.Msi`, `pagefile.sys`, `hiberfil.sys`, `$WinREAgent`,
   `Recovery`, plus `Windows` / `Program Files` / `Users` in case the drive once held another Windows
   install. This list does **not** depend on the user's "show hidden / system files" setting.
3. **Application folders are left in place** — detected via uninstallers (`unins*.exe`), Steam
   libraries, Electron/Chromium structure, or an `.exe` shipping alongside 3+ `.dll` files. Moving
   these would break registry paths and shortcuts.
4. **Junctions / symlinks are not followed** — a reparse point like `D:\OneDrive` is skipped, avoiding
   infinite loops and cloud-download triggers.

When scanning a whole drive, the Analytics screen adds: a usage bar split three ways (in scope /
protected / free) and a table of the **top-level folders taking the most space** — one glance tells
you what's eating the drive.

**Sort photos by where they were taken** — reads GPS coordinates embedded in the photo (EXIF), then
looks up the city name fully offline, creating folders like `Da-Nang`, `Tokyo`, `Munich`… A table of
**~34,000 cities worldwide** (GeoNames data, population over 15,000) is embedded directly in the
executable as a compact binary (~730KB), queried through a 1° grid so it stays fast despite the count.

Two rules to produce the name the user actually expects:
- **Prefer the largest city within 30km**, not the mechanically-nearest point. Otherwise a photo taken
  in central Hanoi would come out as a ward name (`Yen-Phu`) instead of `Hanoi`, because the ward is closer.
- **150km safety cap**: farther than that, it doesn't guess — the file goes to `Khong-Ro-Noi-Chup`
  ("location unknown").

Only phone photos with location enabled carry GPS. City data from
[GeoNames](https://www.geonames.org), licensed CC BY 4.0.

**Batch rename** — assemble a name template from pieces (date, original name, folder name, sequence
number, typed text) with optional cleanup (strip diacritics, lowercase, kebab-case). A live preview
updates as you build it.

Because renaming is just moving a file within the same folder, it reuses the whole journal + undo
engine, so **one button restores every original name exactly**, even after a crash. Three hard safety
layers: the file extension (.jpg, .pdf) is always kept and the user can't change it; an empty name
falls back to the original; Windows-forbidden characters and forbidden device names are sanitized.
Two outputs with the same name get a number appended — they never overwrite each other. Sequence
numbers are assigned per folder and are stable, so the preview matches the real run.

**Smart constraints:**

- `cleanEmptyDirs` — after pulling files out of subfolders, the empty shells left behind are cleaned
  up (post-order traversal, inside-out). It never touches the folder the user selected, never touches
  system items, never follows shortcuts. Every removed folder is journaled so Undo rebuilds the exact
  old structure. Only runs in Move mode.
- `keepSidecarTogether` — RAW+JPG, mp4+srt, psd+preview always stay in the same folder.
- `treatProjectFoldersAsUnit` — a folder with `.git` / `package.json` / `Cargo.toml` is moved as one
  block, not torn apart.
- `protectInstalledApps` — installed-app folders are left in place.

**Duplicates** — 3-tier filter: size → fast hash of the first+last 8KB → full-file BLAKE3
(multithreaded). 4 strategies for which copy to keep, 3 ways to handle the extras.

**Near-duplicate images** — dHash 64-bit perceptual hashing: shrink the image to 9×8 grayscale then
compare adjacent pixel pairs. Catches the same picture saved multiple times at different sizes or
compression levels — something byte-hashing completely misses. Measured on real photos: a 1200×900
version and a 400×300 re-compressed JPEG come out **0 bits apart**, while a genuinely different image
is 36 bits apart.

Grouping is done with union-find over pairs below the threshold, plus an aspect-ratio guard so a
landscape photo never merges with a portrait. The kept copy is the one with the most pixels.

Safety constraint: this is a **guess**, not an exact match. Extras are always moved to a dedicated
`_Similar-Images` folder for the user to review — **never** to the Recycle Bin, even if the user set
that for exact duplicates. Their label in the list also uses a distinctly different color.

**Operation modes:** `MOVE` · `COPY` · `HARDLINK` (a new structure costing zero extra bytes) · `REPORT_ONLY`.

---

## Not yet implemented (Phases 2–4 of the spec)

Rules Engine (if/then) · Watcher & Scheduler · Retention policy · CLI · `AI_SEMANTIC`.

---

## Where data is stored

```
%APPDATA%\Foldu\
├── settings.json     File-type groups, keywords, noise words, recent folders
├── journal\          Per-session log (.jsonl) — used for undo
├── profiles\         Exported config profiles for sharing
└── reports\          HTML / CSV reports
```

Uninstalling the portable build = delete the `.exe` and the folder above, leaving nothing in the registry. The installer build uninstalls through "Apps & features" like any normal program. Full detail on data and privacy: [PRIVACY.en.md](PRIVACY.en.md).

---

## Differences from the spec

| Spec | Actual | Why |
|---|---|---|
| SQLite (`rusqlite`) for the journal | Append-only JSONL | Append-only is safer on crash (one bad line doesn't corrupt the file), no C compilation, faster startup |
| React + Tailwind + shadcn/ui | Plain HTML/CSS/JS | No build step, achieves exactly the intended aesthetic, less toolchain risk |
| `jwalk` | Hand-written walker | Needs early-stop at project folders and reparse-point control — custom logic is easier than fighting a library |

---

## Author & license

**Foldu** — a personal, open-source project, free for everyone.

- **Author / project owner:** Tran Duy Thuan — <https://tranduythuan.com>
- **Code written by:** Claude (Anthropic)
- **License:** [MIT](LICENSE) — © 2026 Tran Duy Thuan. You are free to use, modify, and share.

City data for the photo-location feature comes from [GeoNames](https://www.geonames.org),
licensed **CC BY 4.0**.
