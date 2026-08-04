# Foldu does not send your data anywhere

[Tiếng Việt](PRIVACY.md) · **English**

This is a free, open-source app written by one person. You have no reason to trust me just because I say so. So this document is not a list of promises — **every claim here is something you can verify yourself.**

---

## The short version

Foldu **has no networking capability at all**. Not "I promise not to send your data" — there is simply **no part of the program that could send it.**

---

## Don't trust me — check

Ordered from easiest to most conclusive. Step 1 alone is enough for most people.

### 1. Block it in Windows Firewall, then use it

The most convincing test, and anyone can do it:

1. Press Start, type **Windows Defender Firewall with Advanced Security**, open it
2. Choose **Outbound Rules** → **New Rule…**
3. Choose **Program** → Next → point it at your `foldu.exe`
4. Choose **Block the connection** → Next → Next → name it anything → Finish

Windows now flatly forbids Foldu from reaching the internet. Open it and use **every feature**: scan, sort, find duplicates, review similar photos, sort photos by location, undo.

**Everything still works, with nothing missing.** Because it never needed the network in the first place.

### 2. Watch the network traffic yourself

Press `Windows + R`, type `resmon`, open the **Network** tab. Run Foldu and put it to work. `foldu.exe` will **never appear** among the processes with network activity.

### 3. Read the dependency list — it is 13 lines long

Every library Foldu uses is listed in [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml). Here is the complete list, nothing hidden:

| Library | What it does |
|---|---|
| `tauri` | Builds the app window (`features = []` — no extensions enabled) |
| `serde`, `serde_json` | Reads and writes config files |
| `rayon` | Spreads work across CPU cores |
| `blake3` | Hashes file contents to find exact duplicates |
| `chrono` | Date and time handling |
| `trash` | Sends files to the Windows Recycle Bin |
| `kamadak-exif` | Reads capture date, camera, and GPS from photos |
| `imagesize`, `image` | Reads image dimensions and decodes images |
| `unicode-normalization` | Strips Vietnamese accents when naming folders |
| `once_cell` | A small Rust plumbing detail |
| `rfd` | The native Windows folder picker |

**There is no networking library** — no `reqwest`, no `hyper`, no `ureq`, nothing. A Rust program that wants to send data needs one of those, or hand-written socket code. Search the source for either and you will not find it.

### 4. Read the source

All of it is public under the MIT license in this repository. Nothing is withheld, no closed-source blobs.

### 5. Build it yourself

The conclusive option: follow "Build from source" in the [README](README.en.md#build-from-source). The `.exe` is then one you produced from the code you just read.

---

## So how do the "smart" features work offline?

This is the best question, because people assume recognising photos requires AI or a server. It doesn't. **The understanding lives in the arithmetic, not in data that has to be fetched.**

### Finding near-identical photos

Foldu shrinks each photo to **9×8 = 72 grey dots**, then walks each row asking *"is this dot brighter than the one to its right?"*. Those 64 yes/no answers are the photo's fingerprint.

The key idea: it records the **relationship between neighbouring dots**, not absolute brightness. So resizing, recompressing, brightening or darkening the photo mostly leaves those relationships intact. Comparing two photos is just counting how many of the 64 answers differ — that is the **"N bits apart"** number shown in the app.

An analogy: it is how you recognise two recordings of the same song by humming the tune and comparing. No music library needed; the tune is already in the recording.

Source: [`src-tauri/src/phash.rs`](src-tauri/src/phash.rs)

### Sorting photos by where they were taken

Phones write GPS coordinates into the photo. Foldu turns coordinates into a city name using a table of **~34,000 cities worldwide embedded directly in the `.exe`** (about 730KB, data from [GeoNames](https://www.geonames.org), CC BY 4.0). No mapping service is contacted.

Source: [`src-tauri/src/geo.rs`](src-tauri/src/geo.rs)

### Finding exact duplicates

BLAKE3 hashes of the file contents, compared against each other. Pure computation on your own machine.

### No AI, no downloaded models

Foldu does **not** recognise photo content like *"this one has a cat in it"*. That would require a trained model. It is a line I deliberately do not cross, because it would break this very promise.

---

## What Foldu stores on your computer

All of it lives in **one folder**: `%APPDATA%\Foldu\`
(paste `%APPDATA%\Foldu` into the File Explorer address bar)

| Location | Contents | Why it exists |
|---|---|---|
| `settings.json` | File-type groups, keywords, recent folders, language, light/dark | Remembers your settings |
| `journal\*.jsonl` | **Full paths of every file moved** (from → to) | Required; without it, undo is impossible |
| `profiles\` | Config profiles you saved | Only if you export one |
| `reports\` | Reports you saved with "Save a report" | Only if you click it |

**To be explicit:** the journal and the reports **contain full file names and paths** from the folder you tidied. There is no way around that — putting files back exactly requires remembering exactly where they were. They stay on your machine and go nowhere.

**To remove every trace:** delete `%APPDATA%\Foldu\`. With the portable build, uninstalling is simply deleting the `.exe` and that folder (see the registry note below).

---

## The only thing that can leak is something you send

Foldu sends nothing, but there is one file **you could share by accident**:

**The exported report (HTML/CSV)** contains the **full names and paths** of every file processed. If you send that report to someone, you are showing them your folder structure and file names. Think before forwarding it.

Two things I deliberately prevented:

- **Thumbnails are never written to disk.** They exist in memory only and vanish when you close the app. (Windows Explorer, by contrast, has *already* cached thumbnails of those photos in `thumbcache` — Foldu leaves less behind.)
- **Photos are never embedded in the report**, precisely because the report is a file that might get sent. The report is text only.

---

## What Foldu does not do

- ❌ No account, no sign-in, no activation
- ❌ No usage telemetry
- ❌ No automatic crash reporting
- ❌ No update checks, no self-updating ([explained below](#why-there-is-no-automatic-updating))
- ❌ No ads, no Pro tier, no upgrade prompts
- ❌ Never reads files outside the folder you picked
- ❌ Never needs Administrator rights

**About the registry, precisely:** the portable `foldu.exe` **writes nothing to the registry**. If you use the **installer**, it registers one uninstall entry (under `HKEY_CURRENT_USER`, not machine-wide) — exactly as any Windows program does, so Foldu shows up in "Apps & features" for you to remove. Uninstalling removes that entry too. If you want nothing in the registry at all, use the portable `foldu.exe`.

---

## What permissions it actually needs

Exactly three, all the minimum for the job:

1. **Read and write files in the folder you choose** — to sort them
2. **The Windows Recycle Bin** — the only place spare files ever go; Foldu never deletes permanently
3. **The folder picker dialog** — so you can point at a folder

One technical detail worth stating for careful readers: Foldu **does not enable Tauri's `asset:` protocol** (the protocol that lets the UI layer read files straight off disk). There is no `capabilities/` directory in the project — you can check. Thumbnails are produced by the core and handed to the UI as embedded data, so the UI layer has no disk-read permission at all.

---

## Why there is no automatic updating

Almost every app quietly asks a server *"is there a newer version?"* on startup. Foldu **deliberately does not** — this is a decision, not an unfinished feature.

The reason: an update check sends only your IP address and the time of day (never file names or photos), but it destroys the most valuable thing in this document — **your ability to verify the claim yourself.**

Right now the promise reads *"no networking library exists in this program, so it cannot send anything"* — you can confirm that by opening `Cargo.toml` in five seconds. Add an update check and it has to become *"it only contacts this one address"* — which you can only confirm by reading and understanding the code paths. The burden shifts from **you can check** to **you have to trust me**. That trade is not worth making.

**So how do you find out about new versions?** In the app, go to **Settings → Updates**:

- It shows **the version you are running**
- Click **"Open the downloads page"** — this opens **your browser** at the releases page, so you can compare at a glance

That button is not a network call by Foldu. It only launches your browser process — the same mechanism the "Open the folder" button uses to launch Explorer. Your browser is software you already trust and control; Foldu still sends nothing. Source: the `open_releases` function in [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs), with the address hardcoded in the core so the UI layer cannot make it open anything else.

If you want to be notified automatically, press **Watch → Releases only** on the [GitHub page](https://github.com/tranduythuan/Foldu) — GitHub emails you, while the app on your machine stays completely silent.

---

## One thing I will state plainly, for fairness

Foldu uses **WebView2** to draw its interface — a component that ships with Windows (alongside Microsoft Edge), not something I wrote. Foldu only loads its own local HTML/CSS/JS into it, under a `default-src 'self'` security policy that prevents the page from fetching anything external.

That said, **WebView2 is Microsoft software with its own update mechanism, tied to Windows and Edge** — outside Foldu's control and mine. If you block `foldu.exe` in the firewall as described above, the Foldu process cannot reach out, and that is what I can guarantee.

---

## The SmartScreen warning is not a virus

On first launch Windows may show a blue *"Windows protected your PC"* box. The reason: this free app is **not code-signed** (a certificate costs a few hundred dollars a year). Windows warns about every unsigned program, clean or not.

Click **More info → Run anyway**.

To confirm the file you downloaded is the one I published, every build on the [Releases](https://github.com/tranduythuan/Foldu/releases) page lists a **SHA-256** checksum. Open PowerShell and run:

```bash
Get-FileHash foldu.exe -Algorithm SHA256
```

Compare the result with the value on the Releases page. A match means the file is intact and unmodified.

---

## If you are still unsure

That is a reasonable position. The conclusive route, requiring trust in nobody:

**Block `foldu.exe` in the firewall** (step 1) and **build it yourself from source** (step 5). Then you no longer need to take my word for it — you have proof.

If anything in this document does not match the source code, please open an [issue](https://github.com/tranduythuan/Foldu/issues) and I will fix it.

---

*Part of the [Foldu](README.en.md) project — Tran Duy Thuan, <https://tranduythuan.com>. MIT licensed.*
