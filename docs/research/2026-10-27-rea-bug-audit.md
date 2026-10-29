# PurgeKit bug audit driven by Geek Uninstaller 1.6 and WizTree 4.33 REA

**Date:** 2026-10-27 · **Method:** read-only code audit of PurgeKit plus the REA artifacts produced on 2026-10-26 (Ghidra 12.1.4 pseudocode, string/xref evidence, runtime `cache.dat`/`WinTree3.ini`). No closed-source binary was executed for this audit.

## 1. What the two targets teach

**Geek Uninstaller**

- The process is `requireAdministrator`; uninstall and cleanup run with full rights, which is why it can wait on an uninstaller process tree and clean Program Files/registry entries.
- `0x42fc55` icon finder order: DisplayIcon → Start Menu shortcut → InstallLocation → UninstallString → Windows Installer, with icon results cached (`cache.dat`, empty-icon sentinel).
- `InstallLocationFinder` rejects false-positive install locations; install-location inference is deliberately conservative because a wrong location becomes a deletion target.
- The uninstaller wait tracks the process and its children; PurgeKit mirrors this with a Windows Job Object.

**WizTree**

- Duplicate detection is **name + size**, optionally **+ modified date**. WizTree does not compare file contents; grouping is a fast candidate search, not a hash comparison.
- The MFT engine reads `$MFT` directly, needs Administrator for live dumps, and can scan a folder subtree through the same engine.
- File view distinguishes Percent of Parent, Percent of Drive, Items, Folders, Allocated and Dup Count/Dup Size, and offers free-space/allocated treemap options.
- CSV and TSV imports are recognized by `.csv`/`.tsv` and report the failing line when import fails.

## 2. Findings fixed in this pass

| # | Severity | Area | Bug | Fix |
|---|---|---|---|---|
| 1 | High | Store uninstall | Elevated scans emit `Remove-AppxPackage -AllUsers -Package …`, but both single and bulk uninstall only recognized the variant without `-AllUsers`; elevated Store removal failed and could fall through to the shell-wrapper rejection. | Shared `parse_uwp_uninstall_command` accepts both forms and the command runs in the Job Object. |
| 2 | High | Elevated uninstallers | A user-writable uninstaller outside `%USERPROFILE%` (for example `C:\Temp\payload.exe` or `C:\Users\Public\…`) could run elevated without a signature check. | Absolute-path uninstallers must live under Windows/Program Files or carry a valid signature when PurgeKit is elevated. |
| 3 | High | Remnant scans | The recursive remnant walk followed junctions/symlinks, so a matching name could resolve outside the intended tree. | Reparse points and symlinks are skipped for both matching and recursion. |
| 4 | High | Failed uninstall | After an uninstaller failed or was cancelled, leftovers were still scanned, auto-selected and (in bulk mode) auto-purged while the app could still be installed. | Leftovers are not pre-selected or auto-purged when the uninstaller did not complete; the UI requires an explicit review. |
| 5 | High | Restore quarantine | Restoring onto an existing path silently overwrote files through the copy fallback. | Restore is refused when the original path already exists; the rollback path in `backup.rs` uses the same rule. |
| 6 | High | Scheduled tasks | Windows servicing tasks under `\Microsoft\Windows\` were listed as deletable startup items. | They are filtered from the list and rejected by status/delete commands. |
| 7 | Med-High | Dev cache cleaner | Dynamic cache commands (`npm config get cache`, …) were trusted and `remove_dir_all`-ed anywhere safe-to-delete, and freed bytes were over-reported when nothing was removed. | Dynamic paths must stay under the rule's template roots, removal errors propagate, and freed bytes are measured. |
| 8 | Med | File search after MFT | MFT roots keep a trailing separator, while search/export canonicalized the lookup without it, so folder MFT scans and `.pkmft` dumps returned “Scan this path before searching”. | Cache keys normalize trailing separators on both store and lookup. |
| 9 | Med | Install tracking | `stop_install_tracking` removed the baseline before writing the snapshot; a write/DB error lost it. | The session is cloned and cleared only after the snapshot is durable. |
| 10 | Med | Shortcut matching | `matches_target` still matched a target by app name when an install location was set, so force-removing short names (for example “Code”) could delete other Start-Menu shortcuts. | When a location is present only location-bound targets match. |
| 11 | Med | Admin detection | Remnant admin checks and quarantine restore used `contains("WINDOWS")`/`contains("PROGRAM FILES")`, blocking normal user paths such as `…\Windows Themes`. | Boundary-aware path checks are shared by both flows. |
| 12 | Med | Store identity | Store rows showed package-derived names (`10511Claudia Wey inknotespro`) and publisher IDs. | Localized Start-menu names and `PublisherDisplayName` are used when available. |
| 13 | Med | Snapshot hygiene | Snapshots scanned `%APPDATA%\PurgeKit` and `%LOCALAPPDATA%\PurgeKit`, so they contained their own JSON files and quarantine/backup data. | PurgeKit data folders are excluded and reparse points are skipped. |
| 14 | Med | UWP icons | A fully transparent Store logo PNG passed the “icon exists” check, so stub packages rendered a blank square. | PNG alpha bytes are inspected and invisible logos fall through to the executable/bounded walk. |
| 15 | Low-Med | Theme contrast | Hardcoded `emerald/blue/amber/rose` classes and solid accent backgrounds produced weak badge text and washed-out button labels, especially in dark mode. | Semantic `success/info/warning/danger` tokens plus `*-strong` action colors; muted/secondary/border tokens were raised in both palettes. |
| 16 | Med | Project deletion authority | The delete command accepted any path that passed the generic safety gate, so a compromised webview could remove an arbitrary non-protected folder. | Deletion is restricted to folders returned by the most recent project scan; unscanned paths fail without touching disk. |
| 17 | Med | Large tree view | The 20k-node tree budget was spent depth-first; on a real 3.15 GB WizTree dump one huge subtree consumed it and only 2 of 49 top-level folders appeared. | Breadth-first budget with a regression test; all root folders are represented before deeper detail. |
| 18 | Med | MFT file sizes | FILE_NAME sizes were stale/zero for hardlinked/system files, while named streams such as `$BadClus::$Bad` inflated totals to the whole volume. | Logical/allocated sizes now come from the unnamed `$DATA` attribute, with FILE_NAME only as fallback. |
| 19 | High | Elevated MFT scan | The `ntfs` crate reads the boot sector in 3–8 byte chunks; a raw volume handle rejects unaligned reads with `ERROR_INVALID_PARAMETER` and the crate panics. | A 64 KB aligned block-cache reader serves arbitrary reads; the live elevated C: dump now completes. |

## 3. Follow-up WizTree features implemented

- **Treemap controls:** label toggle, allocated/logical weighting, free-space tile at drive roots (using the volume's reported free bytes), and grouping files below 1 KB / 64 KB / 1 MB. All options persist with the disk preferences; synthetic free/small tiles cannot be zoomed or quarantined.
- **HTML report export:** self-contained printable report with summary, top 100 files/folders, file types and top duplicate groups.
- **Bulk result reporting:** the backend returns a failed-app count, and the UI only reports success when every app succeeded; partial purge results are surfaced.
- **Startup Manager icons:** Run values and Startup-folder shortcuts resolve their executable/shortcut target and render the real icon with the shared frame treatment. Absolute targets that no longer exist are flagged so a stale entry is not mistaken for a missing-icon bug.
- **WizTree export options:** CSV/TSV, current-view/all scope, optional folder totals and created/last-accessed dates, verified against WizTree’s shipped English locale strings.
- **Geek install date:** registry dates are normalized and inferred from install-folder/uninstaller creation time when missing, then cached and exported.
- **Treemap navigation:** double-click zooms a file into its parent and an Up control walks back out.
- **WizTree `.MFT` import:** raw records plus the `VOLDATA2`/`NTFS_VOLUME_DATA_BUFFER`/drive-letter footer are loaded read-only, with volume size and MFT record numbers available to search and export.
- **Large-dump fixes:** breadth-first tree budgeting keeps every top-level folder, and MFT file sizes use unnamed `$DATA` streams instead of stale FILE_NAME sizes or volume-sized named streams.
- **Aligned volume reader:** boot-sector and NTFS metadata reads are served through a 64 KB aligned block cache, fixing the elevated `ERROR_INVALID_PARAMETER` panic. Buffered gzip JSON reduced the 42 MB `.pkmft` reload from >15 minutes to ~77 s in the debug build.

## 4. Deliberately deferred

- **Content hashing for duplicates.** WizTree groups by name/size/date and does not read file contents; hashing would change performance and semantics. The UI now states clearly that contents are not compared and asks the user to verify before purging.
- **Percent of Parent / Percent of Drive / Items / Folders columns and MFT/CSV schema compatibility.** These are WizTree features, not correctness bugs. PurgeKit’s current equivalents are Share %, file counts, allocated size, CSV import/export and `.pkmft` dump/load.
- **Provisioned Store packages** remain out of scope: enumerating or removing them can affect other users and servicing.
- **Live elevated validation** of folder-MFT, MFT dump and multi-process uninstall waits still requires an Administrator session.

## 5. Verification

- `cargo test --lib`: **119 passed, 4 ignored** (host smoke tests only).
- `cargo check --bins`, `npm run check`, `npm run build`: clean.
- Host icon coverage: 132/133 inventory rows have a logo; `vs_CoreEditorFonts` is a font component without an application icon. Startup Manager shows icons for 11/15 entries; the remaining four are stale Run values whose targets no longer exist and are flagged with a warning.
- Host install dates: 137/138 inventory rows have a date after the fallback pass (registry, Store package folder, CLI, toolchain and portable paths); one registry entry has no readable install path.
- **Live elevated C: MFT dump succeeded** after the aligned-volume fix: **1,408,925 files / 253,869 folders**, 49 top-level folders, 268.2 GB logical / 271.2 GB allocated, footer volume 403.17 GB with 98.37 GB free; the resulting 42 MB `.pkmft` reloaded as an `mft-dump` tree. A detached-grandchild job-object test also passed in 2.5 s, covering multi-process uninstaller waits. A clean live folder-MFT harness on `C:\Windows` passed in **51.1 s**: 148,058 files / 81,931 folders / 50 children. Earlier “silent exits” were the harness log watcher contending with the result file, not an application crash.
- A real 3,155,165,289-byte WizTree `.MFT` dump was loaded end-to-end: 3,081,216 records, **1,409,381 files / 253,836 folders**, all **49** top-level folders, footer volume **403.17 GB** with **102.71 GB** free. The debug build took ~114 s; the file index/search/export carried MFT record numbers. Selecting the `C:\Windows` subtree from the same dump produced **129,638 files / 72,577 folders / 50 children**. The actual Disk tab UI then loaded the same 3.15 GB file read-only in **183.85 s** before optimization: 1,409,381 files / 253,836 folders and a 375.5 GiB volume with 95.7 GiB free, confirming the UI path and not just the harness. Parallel chunk parsing cut the backend load to **50.7 s** in debug (parse 121.5 s → 35.5 s, tree 11.9 s, finalize 3.3 s) with identical counts; the Disk tab UI then loaded the same file in **52 s**.
- UI verified through WebView2 CDP at 900×600, 1500×1000 and a 1600px emulated viewport: export dialog produced a 26-line TSV with file/folder rows, created/last-accessed, parent-percent, MFT-record and drive-size sections; the file-types view exported a CSV with full type totals; a reconstructed WizTree-style `.MFT` loaded read-only with 10,854 files / 978 folders, a 91.6 GiB volume and 38.1 GiB free, and MFT record 3365 visible in search/export; double-click zoom produced a `research` breadcrumb and Up returned to the root; an unscanned delete request was rejected while the folder stayed in place.
- The UI is English-only for now; the Vietnamese dictionary and `setMode()` remain wired for a later language release.