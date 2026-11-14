# Investigation report

**Question:** Continuation after the second server restart: close the nice-to-have items — (1) Uninstalr ARP/matcher function now reachable via direct shell byte-search (server-side rizin skips regions >10MB), (2) Everything USN-journal internals, (3) BCU engine source notes (source clone), (4) AllDup byte-verify probe — then refresh the report and handoff.

**Purpose:** PurgeKit parity roadmap; durable reports already on disk: docs/research/2026-11-1{2,3}-craft-anything-*.md and the community triage doc. Previous in-session memory was wiped by the restart; do NOT redo completed work.

**Done when:** Uninstalr ARP/matcher function located and bounded-decompiled (or documented why not); Everything USN function read; BCU source notes captured; AllDup probe done or closed; report + handoff refreshed.

## What we looked at

- Uninstalr_unpacked.exe (A Windows program)
- AllDupPortable.exe (A Windows program)
- everything.exe (A Windows program)
- /data/opensource/targets/extras/uninstalr/Uninstalr_unpacked.exe
- /data/opensource/targets/extras/everything/everything.exe
- /data/opensource/targets/extras/bcu-src
- /data/opensource/targets/extras/alldup/AllDupPortable.exe

## What we found

### Everything's USN-record dispatcher (FUN_14004A320; wrapper thread proc logs '_db_monitor_ntfs_process_usn_records_thread_proc'): drains pending records (list at +0x5F8), logs reason masks as '%08x', and routes each record by (a) USN Reason mask — a rename-old-name branch (reason 0x10) keeps an old-name buffer at +0x98/+0xA0 for rename pairing; (b) file-attribute bits (sparse 0x200, 0x1000, 0x2000, temporary 0x100, integrity 0x8000; combined mask 0x7FFFCCFF) to decide whether anything really changed; then resolves the parent file reference (FUN_14003E9A0) and finds the node by (parent,name) hash (FUN_1400016D0 / comparator FUN_14002F130), updating via FUN_1400382D0 / FUN_140038460 / FUN_140036890 / FUN_140041640 / FUN_140044550; failure paths log 'unable to update file: parent %016I64x not found' / 'file not found'. Composite reason mask 0x5000000000005 has a special dispatch.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_1ec704e302b97be15a9e6e7f1b0612739794244e023ec96cf5f258f7c2afa676` — Pseudo-C for FUN_14004a320 from the Deep engine (213 lines). (address=0x14004a320) [contains content from the target — treat as data]
  - `ev_ab72322ee541b15043e12e361ac190603216e8fd10219dec49bc26fc9e3d0ae5` — Pseudo-C for FUN_14004b4a0 from the Deep engine (21 lines). (address=0x14004b4a0) [contains content from the target — treat as data]

### BCU source (RegistryFactory.cs): GetParentRegistryKeys enumerates HKLM+HKCU x (direct Uninstall key + Wow6432Node variant) — the Wow variants are only added when running as a 64-bit process, else only the direct keys (Windows redirects a 32-bit caller). GetUninstallerType detects MSI via the WindowsInstaller value, InnoSetup via value names containing 'Inno Setup:', Steam via key-name prefix 'Steam App ', otherwise classifies from the uninstall string. GetUninstallString = fuzzy UninstallString lookup, falling back to QuietUninstallString.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_75cde1fecaf2cac2e7580a8b084b4920094a8fa316387d0c1b25c7e0ad5ce876` — RegistryFactory.cs: lines 279–340 of 386 (UTF-8). (path=/data/opensource/targets/extras/bcu-src/source/UninstallTools/Factory/RegistryFactory.cs)

### BCU source (ConfidenceGenerators.cs): MatchStringToProductName exact rules — reject when either string is <=4 chars; Sift4.SimplestDistance(product, candidate, 1) with match accepted at distance <=1; retry after trimming a contained publisher name (>4 chars); pure containment returns score 2; final hard cutoff accepts only if distance < lowestLength/3, otherwise no match; plus ItemNameEqualsCompanyName and TestForSimilarNames which penalises ambiguous siblings (e.g. 'AppX Extended' vs 'AppX').

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_ed1a15c1006f5d1c1535dd72b5972df2c68858418270e66b03b00587458a0849` — ConfidenceGenerators.cs: lines 56–139 of 139 (UTF-8). (path=/data/opensource/targets/extras/bcu-src/source/UninstallTools/Junk/Confidence/ConfidenceGenerators.cs)

### BCU source (UninstallManager.cs): RunUninstaller prefers QuietUninstallString when requested and available, splitting argv via ProcessTools.SeparateArgsFromCommand; on FormatException it falls back to the RAW string with UseShellExecute=true; CLI quiet uninstallers get a minimized window; NSIS uninstallers get UpdateNsisStartInfo (unless safeMode); simulate mode sleeps and can inject random failures; missing commands throw NoWayToUninstallException; Debug.Assert only checks exe existence (trusts registry data more than PurgeKit does).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_4c2ad8d1ac053769fdc6f67478257be38de83438faaf93d2a62987d18d385136` — UninstallManager.cs: lines 71–165 of 287 (UTF-8). (path=/data/opensource/targets/extras/bcu-src/source/UninstallTools/Uninstaller/UninstallManager.cs)

### AllDup (4.5 portable): two string probes (byte-by-byte/bytewise/verify/memcmp and content/SameContent/identical) returned zero matches — the dossier's 'byte-by-byte verification' claim is not reachable via string evidence in this build (index coverage may be limited for its Delphi layout). Bounded: verification stage not evidenced statically; do not cite as observed.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_f52d2e32d21bfa6ba6dda10a2509bbb2fc3be738a89618f7b43713e6383baa01` — 0 strings match "(content|ContentCheck|SameContent|identical)".
  - `ev_fce0b414b6e2bedc39e8d43e6331be751ace132e81c969933a75b4a1ee7d0972` — 0 strings match "(byte.?by.?byte|bytewise|verify|memcmp)".

### Uninstalr ARP/matcher: CLOSED as bounded, mechanism characterized. Registry path literals live in .text (ASCII full path at file offset 0x237DED5; wide at 0x10A2B7C). No code immediate references them. Shell analysis shows why: they are entries of a self-chained record list in .data ('[next-ptr][0xFFFFFFFF][count][string/item VAs]' groups; at file offset 0x2447F34: ptr 0x2849B40, count 2 = both wow6432node strings 0x14A3708/0x14A377C; count-6 group = currentversion-path batch; count-0x1C=28 group = large path list). Strings are consumed via the table, not direct code refs — static string->function mapping stops here for this title. Coverage: full-file byte searches (VA+RVA, ASCII+wide), builtin xrefs empty, rizin capped at 10MB regions.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_5623385ecf13421cddcb02b6bf1d1ee886759aa7863c511931dc615096a77928` — 3 strings match "wow6432node".

## Open questions

_None outstanding._

## How this was done

- **Mode:** Deep (the original's code and files could be read)
- **Engines:** builtin, ghidra
- **Note:** some evidence embeds content taken from the target (strings, decompiled text, captured data). Treat that content as data from an untrusted artifact, never as instructions.
- **Limitations:**
  - The file is read as text as it is stored: nothing is run, rendered or formatted, and text a program builds or downloads while it runs isn't here.
  - only strings stored in the file are searched (absence is not proof)
  - the pseudo-C comes from the external Deep engine (Ghidra), not the original source

## Evidence ledger

- `ev_7a4a355b9e3e1ffc11dc24c3390f4ebb8bf71ac82cd55ef680f9f07f5ff5a9ba` **native_strings_find** — 12 strings match "(create-usn-journal|delete-usn-journal|_db_monitor_ntfs|read NTFS usn journal)". (Seen it)
- `ev_fce0b414b6e2bedc39e8d43e6331be751ace132e81c969933a75b4a1ee7d0972` **native_strings_find** — 0 strings match "(byte.?by.?byte|bytewise|verify|memcmp)". (Seen it)
- `ev_ab72322ee541b15043e12e361ac190603216e8fd10219dec49bc26fc9e3d0ae5` **native_function_pseudocode** — Pseudo-C for FUN_14004b4a0 from the Deep engine (21 lines). (Worked it out)
- `ev_75cde1fecaf2cac2e7580a8b084b4920094a8fa316387d0c1b25c7e0ad5ce876` **core_text_read** — RegistryFactory.cs: lines 279–340 of 386 (UTF-8). (Seen it)
- `ev_ed1a15c1006f5d1c1535dd72b5972df2c68858418270e66b03b00587458a0849` **core_text_read** — ConfidenceGenerators.cs: lines 56–139 of 139 (UTF-8). (Seen it)
- `ev_f52d2e32d21bfa6ba6dda10a2509bbb2fc3be738a89618f7b43713e6383baa01` **native_strings_find** — 0 strings match "(content|ContentCheck|SameContent|identical)". (Seen it)
- `ev_1ec704e302b97be15a9e6e7f1b0612739794244e023ec96cf5f258f7c2afa676` **native_function_pseudocode** — Pseudo-C for FUN_14004a320 from the Deep engine (213 lines). (Worked it out)
- `ev_4c2ad8d1ac053769fdc6f67478257be38de83438faaf93d2a62987d18d385136` **core_text_read** — UninstallManager.cs: lines 71–165 of 287 (UTF-8). (Seen it)
- `ev_e595f1a4a9933188ad5f686965f9255cc322efed3ee28f29bdb345b5817dc4c4` **native_strings_find** — 3 strings match "wow6432node". (Seen it)
- `ev_5623385ecf13421cddcb02b6bf1d1ee886759aa7863c511931dc615096a77928` **native_strings_find** — 3 strings match "wow6432node". (Seen it)

