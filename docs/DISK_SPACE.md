# 💽 Disk Space & MFT Scanner

The Disk Space workspace is PurgeKit's WizTree-style analyzer. It combines a fast NTFS MFT path with a traversal fallback, a nested treemap, file-level views, duplicate candidates and portable reports.

---

## 📋 Requirements

* Windows 10/11 on a local NTFS volume for the MFT fast path.
* **Administrator** for live MFT scans and `Dump MFT`.
* A normal user session can still use traversal scans, CSV/TSV imports and saved dumps.
* Format `.MFT` files written by WizTree are read-only in PurgeKit.

---

## 🔍 Scanning

1. Open **Disk Space**.
2. Enter a drive root (`C:\`) or folder (`D:\Code`). Multiple roots are separated with semicolons (`C:\; D:\Games`).
3. Choose the duplicate mode: **Off**, **Name + size**, or **Name + size + date**.
4. Press **Scan**. When elevated and local NTFS is available, the engine reads `$MFT` directly; otherwise it falls back to a bounded traversal.
5. Use **Cancel** for long scans. Progress reports parsed files, folders and bytes.

Each scan reports:

| Field | Meaning |
|---|---|
| Allocated | Space occupied on disk (sparse/compressed aware) |
| Logical | Sum of file lengths |
| Files / folders | Counts in the scanned tree |
| Volume free | Current free space on the target volume |
| Engine | `MFT FAST SCAN`, `TRAVERSAL FALLBACK`, `CSV IMPORT`, `MFT DUMP`, `MFT FILE` or `MULTI-ROOT` |

---

## 🧭 Views

* **Treemap** — nested tiles weighted by allocated or logical size; labels, free-space tile, small-file grouping, click/double-click zoom and Up/breadcrumb navigation.
* **Largest files** — sortable/searchable file list with extension, modified time, attributes, share %, allocated and logical sizes.
* **File types** — allocated size and count per extension; export the complete type table as CSV.
* **Duplicates** — candidate groups by name + size (optionally modified date). File contents are **not** hashed or compared.

Filters support name-only or full-path matching, extension, include/exclude substrings, folder scope and a maximum result count.

---

## 🗄️ MFT Dumps

### PurgeKit `.pkmft`

Admin scans can write a compressed snapshot of the parsed tree. It reloads without Administrator as a read-only view; quarantine and cleanup stay disabled until a live rescan.

### WizTree `.MFT`

WizTree writes raw MFT records followed by a `VOLDATA2` footer containing an `NTFS_VOLUME_DATA_BUFFER` and the drive letter. PurgeKit imports that layout, accepts plain raw MFT dumps, and can resolve a folder subtree inside the dump. The footer supplies volume size, free space and the default drive letter.

The importer rejects files that do not contain `FILE`/`BAAD` records and validates the selected drive against the footer.

---

## 📤 Exports

Disk exports support CSV or TSV and can include:

* Current view or all matching files (100,000-file cap).
* Direct file totals per folder.
* Created and last-accessed timestamps.
* Percentage of parent.
* NTFS MFT record numbers (MFT scans).
* Drive total/free/used bytes.

Additional outputs: full file-types CSV, treemap PNG and a self-contained HTML report.

---

## ⚡ Performance Notes

* Large `.MFT` imports are parsed in parallel chunks. A 3.15 GB WizTree dump (3,081,216 records) loaded in about **52 s** in the debug UI with 1,409,381 files and 253,836 folders.
* Live MFT scans read volume data through a 64 KB aligned block cache because raw volume handles reject small unaligned reads.
* The treemap node budget is spent breadth-first so a single huge subtree cannot hide the drive's other top-level folders.
* Results are capped for interactive use (400,000 indexed files / 20,000 tree nodes); the UI labels truncated results.

---

## 🔐 Safety Behavior

* Imported dumps are read-only; quarantine is unavailable.
* Live MFT/dump operations require elevation and never modify the volume.
* Quarantine/delete actions from scan results still pass the central safe-path gate.
* Symbols, junctions and reparse points are skipped during traversal and snapshots.

See [the REA parity report](research/2026-10-26-rea-geek-1.6-wiztree-4.33.md) for observed WizTree behavior and implementation evidence.
