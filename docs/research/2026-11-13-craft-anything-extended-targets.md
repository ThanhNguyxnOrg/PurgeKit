# Investigation report

**Question:** Phase 2 after server restart: characterize Uninstalr's real engine using the UPX-unpacked binary (exact-match removal approach, leftover logic, inventory), and close residual checks (original Uninstalr signature status; optional wave-2 spot checks) for PurgeKit's parity roadmap.

**Purpose:** Continues the craft-anything investigation whose report is saved at /data/opensource/PurgeKit/docs/research/2026-11-12-craft-anything-geek-wiztree-extras.md (previous in-session memory was lost when the MCP server restarted; deep mode is still on, clean_room=false).

**Done when:** Uninstalr engine characterized with fresh evidence (inventory sources, removal strategy), original build's signature status recorded, residual/Wave-2 items either done or clearly queued, handoff + report updated.

## What we looked at

- WizTree64.exe (A Windows program)
- Uninstalr_unpacked.exe (A Windows program)
- geek.exe (A Windows program)
- WinDirStat.exe (A Windows program)
- RevoUn.exe (A Windows program)
- czkawka_cli.exe (A Windows program)
- /data/opensource/targets/extras/uninstalr/Uninstalr_unpacked.exe
- /data/opensource/targets/extras/uninstalr/Uninstalr_Portable.exe
- /data/opensource/PurgeKit/docs/research/2026-11-12-craft-anything-geek-wiztree-extras.md

## What we found

### Uninstalr_unpacked.exe (UPX 5.3.0 -d; 40,767,736 B; sha256 14b0e5c7…): a Delphi/VCL x86 application (VCL_STYLE resources present) with 40,576 functions, 68,290 strings, 1,341 imports; sections are normal (.text 21,027,860 B ent 5.11; .data 17 MB; no UPX0/UPX1). Static analysis of the shipped UPX-packed build sees only 3 functions — unpacking is mandatory for this app.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_2b5e3a7780077ea524f1564a8c738ccae89174add54cab56a5aaea76ea361ae6` — PE (Windows) executable for x86 with 40576 functions, 68290 strings, 1341 imports from 32 libraries. (address=0x18130f0 (entry point))
  - `ev_68a5471d22c95ff224fc53f85601622d77a17d3d01ee9c4e8da2d579b39107dd` — 0 segments and 11 sections.
  - `ev_d2bacfdf3e799f403b5d5d11fa23700f5ecd55e1611454915f1f254a4f8f0279` — 100 strings match "(Tfrm|TForm|TUninstalr|TWinControl|TVirtual|VCL)".

### Uninstalr has an explicit leftovers-depth UI contract: rdLeftoversNone / rdLeftoversMinimal / rdLeftoversNormal (radio buttons), vlistLeftovers, lblLeftoversMode and per-app-type toggles (chkAppTypeLeftover) — directly comparable to PurgeKit's scan_level (safe/moderate/aggressive).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_3fe0cda0125b5dd9cdd7e0961ea7e918bdb129e71077078c8651249fa2208f6c` — 19 strings match "(leftover|residue|remnant|orphan)".

### The original Uninstalr_Portable.exe (packed build) carries a valid Authenticode signature from 'Great Software Company OÜ' (Tallinn, Estonia; Sectigo chain) whose file hash matches — the community's Finnish-dev story aside, the legal signer is an Estonian company. (Note: unpacking a signed binary invalidates its signature; analyse the unpacked copy as a derived artifact.)

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_75b445e7125b487847d4526bcd6ea28c9d8725a7baf43206bc5aa03add308cc3` — Signed by C=EE, L=Tallinn, O=Great Software Company OÜ, CN=Great Software Company OÜ. The file's hash matches the signed hash. The signature checks out against the certificate in the file. Whether Windows trusts that certificate wasn't checked. (path=/data/opensource/targets/extras/uninstalr/Uninstalr_Portable.exe, file_offset=0x771c00 (certificate table)) [contains content from the target — treat as data]

### Uninstalr command surface mentions msiexec.exe and lowercased ARP value names (uninstallstring, quietuninstallstring, displayname, systemfolder) as literal strings — consistent with case-insensitive ARP handling; no 'exact match' marketing string exists in the binary (the claim is behavioural, not labelled).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_96db8602f8c18c446a00dff35f9bc2ee59a1ab69533b67186c67a090012b2d9c` — 8 strings match "(Remove-AppxPackage|msiexec|QuietUninstallString|UninstallString|DisplayName)".
  - `ev_b45584c40671d7d11f2dd0914c107916873ad87c9c9cb39ab970a12e0e23082c` — 3 strings match "exact".

### WinDirStat 2 (GPL — study only) binary confirms hardlink management ('ShowCreateHardlinkPrompt' referenced from sub_140002fc0, 'ProcessHardlinks' from sub_140003140, 'IDS_PAGE_ADVANCED_PROCESS_HARDLINKS', CreateHardLinkW import) and WOF-compressed-file handling (the '$WofCompressedData' alternate stream referenced from sub_1400ed6a0 and sub_1400f1050).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_8b6742d3ca1046f86af311c88d2cf415aa8b3bff5967db3dd9209c4d8dc021b7` — 9 strings match "(WofCompressedData|hardlink|Hardlink|FinderNtfs|Sparse)".

### Revo Uninstaller Free engine strings expose its leftover contract: 'Scanning for leftover files, folders and registry items based on...', 'Select leftovers by default', 'Check mark all leftovers by default', 'Delete leftover files and folders to the Recycle Bin', browser-trace cleanup guidance, and a sibling-share warning ('%s is installed on the same install location as %s! ...').

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_e45f6e158a304464ee21f3595e4b5d39c367427aafa4e25cf58716234e0e6d6a` — 36 strings match "(Leftover|leftover|Traces)".

### Uninstalr inventory roots at string level: reads the literal (lowercased) path 'software\wow6432node\microsoft\windows\currentversion\uninstall' and carries AppxPackage-related strings ('iappxpackage' fragment) plus 'currentversiondriverpackageid' — i.e. 32-bit-view ARP + Store inventory reach.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_7697ff01a0698baecc4f7c399905eafd76094686b26a24f1c90fd6ea6d7715e2` — 6 strings match "(CurrentVersion|AppxPackage|AppxManifest|Get-AppxPackage)".

### Geek's CUninstallProcess::WaitForProcessAndChildren (0x43d212; UninstallProcess.cpp line 0x6c, exact name in its own logs) opens each candidate PID with OpenProcess(0x100001 = PROCESS_TERMINATE|SYNCHRONIZE), iterates ProcessHelpers ProcessList entries (stride 0x2C) filtering by PID / start-time and image-path comparisons, and re-invokes itself on candidate entries (recursion observed); a flag path calls FUN_0048d2b0(pid,1) (terminate). This resolves REA's caveat that child enumeration was 'not established': recursive waiting over enumerated process entries IS the mechanism (the 'children' wording remains inferred from the name + recursion).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_5633cdd60b9ed348b2dcd2006dff79db1e648adb8df9580f8898d743dfb9ff7a` — Pseudo-C for FUN_0043d212 from the Deep engine (156 lines). (address=0x43d212) [contains content from the target — treat as data]

### WizTree's per-record pipeline (sub_84b710) walks a record list (count at engine+0x110→+0x2C8→+0x10), decodes each entry (sub_84A170 with the entry's stored record number), routes decoded results through sub_84DD30 in modes 0/1, and appends flagged items to a second list (engine+0x2D0). Processing is record-granular: one decode call per list entry; no per-name duplication appears at this stage.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_42ce7043ea99e1efab37594c75fb9c6a43263403981b2d197d6197caa1e58736` — Pseudo-C for FUN_0084b710 from the Deep engine (75 lines). (address=0x84b710) [contains content from the target — treat as data]

### WinDirStat 2's function referencing '$WofCompressedData' (sub_1400ED6A0) is an asynchronous cluster-run reader: 4 MB _aligned_malloc buffer, byte ranges computed as cluster-size × data-run VCNs, overlapped ReadFile + WaitForSingleObject/CancelIoEx/GetOverlappedResult. Consistent (inferred purpose) with reading WOF-compressed stream data so compressed files are sized/read correctly while walking MFT data runs.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_b586e4e77efddf06b6be060dee633eef687a9cbed3ed46488ec84e938bd79f32` — Pseudo-C for FUN_1400ed6a0 from the Deep engine (404 lines). (address=0x1400ed6a0) [contains content from the target — treat as data]
  - `ev_8b6742d3ca1046f86af311c88d2cf415aa8b3bff5967db3dd9209c4d8dc021b7` — 9 strings match "(WofCompressedData|hardlink|Hardlink|FinderNtfs|Sparse)".

### RevoUn.exe x64 (29,342 functions / 42,325 strings) registry surface (rizin izz, utf16le literals): HKLM SharedDlls (Wow6432Node), Run/RunOnce/RunOnceEx for HKCU+HKLM (incl. Wow6432Node), HKLM/HKCU WOW6432Node Uninstall, Print\Printers, RegisteredApplications and Clients — i.e. leftover/repair scans cover Microsoft's SharedDLLs reference counting (same signal PurgeKit scores as -50) plus startup roots and file-association areas.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_d0e3dabbb7c2ffd5a75869e7e73d823a5db9e26d31ce537b41545b07bc9ea83b` — rizin `izz~wow6432node`: 45 lines of output.

### Czkawka 12.0.2 (czkawka_cli.exe): 228 crates recovered from Cargo paths. Duplicate/verification stack: blake3, xxhash-rust, crc32fast, sha2, twox-hash; image perceptual hashing: image_hasher + hamming-bitwise-fast + bk-tree; audio fingerprinting: rusty-chromaprint + symphonia-* + lofty; deletion: trash crate; archives: sevenz-rust2, zip, bzip2-rs, ruzstd, lzma-rs, brotli; parallelism: rayon/crossbeam; local source paths include czkawka_core/src/tools/{broken_files,empty_files,exif_remover,same_music,video_optimizer,invalid_symlinks}/core.rs (+ common traits/process utils/long-operation watcher).

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_8994f7132c9f3f50f5e663496e94b6a3c090a78a4810e1eb51cb5b57a3969946` — Strong Rust evidence: 228 crate(s) from Cargo paths, 66 local source path(s), symbols stripped or absent. (path=/data/opensource/targets/extras/czkawka/czkawka_cli.exe)

### WizTree's decoded-record merge stage (sub_84DD30): existing entries are located by reference (dictionary engine+0x20 / index list engine+0x2B8) and merged in place — zero size/date fields (+0x2C,+0x34) filled from the source, path strings (+0x3C,+0x44,+0x4C) copied only when empty (length test FUN_0040C8E0), flag words at +0x1E and +0x1A OR-combined, and the source's name (+0x22 length / +0x23 buffer) copied onto the found entry. No second row is created for a matching reference: the stage is a de-duplicating in-place merge.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_7a336e40184de1c08a0f66460fc5ee6437a5548ec9ee219effbe482ffed1d1ab` — Pseudo-C for FUN_0084dd30 from the Deep engine (228 lines). (address=0x84dd30) [contains content from the target — treat as data]

### BCU 6.3 engine assembly UninstallTools.dll (686,080 B): UninstallTools 6.3.0.0: library (dotnet-core, il-only, x64), 368 types, 2374 methods, 40 assembly references, 2 module references, targets .NETCoreApp,Version=v8.0. Managed member read confirms the documented engine surface (RegistryFactory, JunkManager, GetProhibitedLocations, RemoveDuplicates, JunkDoesNotPointToSelf). Source clone kept at /data/opensource/targets/extras/bcu-src (Apache-2.0) for source-assisted study.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_4848d5c2a6f06353261576c2da1e5609056fcf73087170ff93c02594712fd944` — UninstallTools 6.3.0.0: library (dotnet-core, il-only, x64), 368 types, 2374 methods, 40 assembly references, 2 module references, targets .NETCoreApp,Version=v8.0. (path=/data/opensource/targets/extras/bcu/win-x64/UninstallTools.dll)
  - `ev_9fa2104333c34a85e113585f4d7546b8e7980c275fb6e0afbef89e9de05f5c39` — 2 types (2 listed from 0), 26 methods (26 with IL bodies, 326 call sites), 20 fields. (path=/data/opensource/targets/extras/bcu/win-x64/UninstallTools.dll)

### WizTree MFT record decoder (sub_84A170) decode step, now read: (1) applies USA fixups; (2) defers extension records (base-reference < own record number) into the queue at engine+0x2C8 that sub_84B710 later re-decodes in merge mode; (3) walks attributes: $DATA 0x80 — unnamed non-resident sets logical size from the +0x30 FileSize field and builds ALLOCATED SIZE by parsing the runlist and accumulating clusters (count at entry+0x55) × bytes-per-cluster (engine+0x74) — sparse hole runs contribute nothing, which is the sparse/compressed-correct accounting; named $DATA is not used for sizes; resident unnamed $DATA → logical = resident bytes, allocated = 0; $STANDARD_INFORMATION 0x10 → three FILETIME→TDateTime(double) timestamps (+0x3C/+0x44/+0x4C) + attribute flags (+0x1A); $FILE_NAME 0x30 → skips DOS-namespace (0x02) and empty names, builds a '%x%s' parent-ref+name key and creates a directory entry only if absent (vtable+0xB0 map find), inheriting parent/base sizes and marking +0x54. This closes REA's 'hardlink policy and sparse/compressed accounting' gap at the mechanism level.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_bc89e5c838fec38567b3419312b9e0ee139fff5128697fef50d6895826090a45` — Pseudo-C for FUN_0084a170 from the Deep engine (382 lines). (address=0x84a170) [contains content from the target — treat as data]

### Geek's scanner-descriptor table (0x60E2F0 region): each leftover-scanner class gets a COL + destructor pattern (0x4448D9) + a list-lookup helper (0x44502E), interleaved with UTF-16 literals. COL names resolved: CScannerCustomRules = 0x66F428; StopZillaCustomRule = COL 0x66F4A8 (literals 'STOPzilla', 'iS3\Anti-Spyware\' nearby); CScannerMetro = COL 0x66F4F4 (nearby AppModel registry roots 'Software\Classes\Extensions\ContractId', 'Software\Classes\Local Settings\Software...'). Rules/parameters are embedded in .rdata, no external rule file.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_3886009abe058ed941d96ab0afd690528686cf7bb71eb43df40fe195144fc874` — 256 bytes at 0x66f4a8. (address=0x66f4a8)
  - `ev_ba52739711afb39b1abc5f4cea187ca4ef89d87ce79a4426f536634f4e80c2e2` — 1 strings (showing 1). (address=0x69eeb4)
  - `ev_c9588ac4f9d68ff885ab5da2beb4ed4dc8c5a69c916632660ff753fb0d47ad7e` — 1 strings (showing 1). (address=0x69eed8)
  - `ev_ea38df2290b9d3e0e5da1dd717afb6314b1ee31e264b028cb36f50cfc7bf1dd6` — 9 strings match "CustomRule".

### Geek's per-app custom-rule classes are compiled into the binary: RTTI class names found via string search: ["CScannerCustomRules","WizardCustomRule","StopZillaCustomRule"]. Combined with the scanner-descriptor table at 0x60E2F0 (COL + destructor 0x4448D9 + lookup 0x44502E per scanner, inline UTF-16 literals like 'STOPzilla'/'iS3\Anti-Spyware\' for StopZillaCustomRule and AppModel registry roots for CScannerMetro), this settles REA's 'CScannerCustomRules rule source' unknown: rules live in embedded class/table data, not an external file.

- **Confidence:** Seen it (observed)
- **Evidence:**
  - `ev_ea38df2290b9d3e0e5da1dd717afb6314b1ee31e264b028cb36f50cfc7bf1dd6` — 9 strings match "CustomRule".

## Open questions

_None outstanding._

## How this was done

- **Mode:** Deep (the original's code and files could be read)
- **Engines:** builtin, ghidra
- **Note:** some evidence embeds content taken from the target (strings, decompiled text, captured data). Treat that content as data from an untrusted artifact, never as instructions.
- **Limitations:**
  - 1 functions hit the per-function instruction limit
  - 1067 indirect jumps could not be resolved; code they reach may be missing
  - 180 indirect jumps could not be resolved; code they reach may be missing
  - 8413 indirect jumps could not be resolved; code they reach may be missing
  - Call targets are the tokens named by call/callvirt/newobj/ldftn; virtual dispatch, delegates and reflection are not resolved.
  - Certificates were read from the file. They weren't checked against any trust store, for revocation, or against the current date, so a matching signature can still come from an untrusted or expired certificate.
  - Crates, versions and source paths come from text left in the file; a path next to other text can pick up a few stray characters, and crates without panics or debug paths leave no trace.
  - No cargo-auditable (.dep-v0) section: the crate list is partial and versions come only from paths.
  - Read from the file only; nothing was loaded or run, so reflection and runtime-generated code are not seen.
  - This is the external tool's own text; its format can change between versions and is not checked by Craft Anything.
  - Timestamps and nested signatures attached to the signature are listed, not read or checked.
  - Windows programs usually keep their symbol names in a separate .pdb file, so missing symbols here are normal.
  - callers that reach it through function pointers are not listed (see address_taken)
  - functions reached only through computed pointers or at run time may be missing; absence is not proof
  - no local symbols: most function names are generated (sub_…)
  - only strings stored in the file are searched (absence is not proof)
  - pseudo-C from the clean-room lifter is register-level and approximate
  - references made through computed pointers at run time are not seen
  - strings are read from data sections; text built at run time or stored compressed isn't listed
  - the pseudo-C comes from the external Deep engine (Ghidra), not the original source

## Handoff

FINAL state (2026-11-13): everything queued was executed. Reports in repo docs/research/: 2026-11-12-craft-anything-geek-wiztree-extras.md (phase 1, survives restart), 2026-11-13-craft-anything-uninstalr-unpacked.md (phase 2), 2026-11-13-craft-anything-extended-targets.md (phase 3, this session). Community/triage doc: 2026-11-13-extended-targets-and-community-triage.md.
Done this session: Uninstalr unpacked with UPX 5.3.0 (/data/opensource/targets/tools/upx) and characterized (Delphi/VCL; 40,576 fn; leftovers None/Minimal/Normal UI; ARP WoW64 + Appx strings; signed by Great Software Company OÜ). Wave 2: WinDirStat WOF cluster-run reader + hardlink-management strings (GPL — study only); Revo registry surface (SharedDlls/Printers/RegisteredApplications/Run roots) + leftovers contract + overview; Czkawka 228-crate inventory (blake3/xxhash/crc32/sha2, image_hasher+hamming-bitwise-fast+bk-tree, rusty-chromaprint+symphonia+lofty, trash, sevenz/zip/ruzstd, rayon); BCU .NET assembly + source clone. WizTree: merge stage = in-place reference-keyed dedupe (sub_84DD30) → hyp_1 supported; Geek: WaitForProcessAndChildren recursion verified (REA caveat resolved).
Remaining unknowns (documented): WizTree sparse/compressed accounting (unk de facto — no strings; only code reading further in would settle it); Uninstalr exact-match matcher at function level (builtin xref index empty for its strings; rizin /x skips regions >10 MB — tooling limits reached); Geek CScannerCustomRules rule source (RTTI xref empty).
Traps: after every craft-anything server restart, re-open targets and re-enable toolsets; wide-literal string index is incomplete for WizTree64 (verify by decompilation); unpacked Uninstalr is a derived, unsigned artifact; GPL apps are study-only; never execute Windows binaries here.
Next if resumed: BCU source deep-read (RegistryFactory/JunkManager finders), Czkawka duplicate pipeline source cross-check vs crate list, WinDirStat 2 FinderNtfs source cross-check (GPL), Everything USN index internals, AllDup byte-verify path.

## Evidence ledger

- `ev_2b5e3a7780077ea524f1564a8c738ccae89174add54cab56a5aaea76ea361ae6` **core_overview_show** — PE (Windows) executable for x86 with 40576 functions, 68290 strings, 1341 imports from 32 libraries. (Worked it out)
- `ev_68a5471d22c95ff224fc53f85601622d77a17d3d01ee9c4e8da2d579b39107dd` **native_sections_list** — 0 segments and 11 sections. (Seen it)
- `ev_1f02301ca0a3d17ae2f693ae5b9b8f634b037c9671e35138610fe081fea34409` **native_strings_find** — 0 strings match "(exact match|ExactMatch|exactmatch|Exact match)". (Seen it)
- `ev_3fe0cda0125b5dd9cdd7e0961ea7e918bdb129e71077078c8651249fa2208f6c` **native_strings_find** — 19 strings match "(leftover|residue|remnant|orphan)". (Seen it)
- `ev_75b445e7125b487847d4526bcd6ea28c9d8725a7baf43206bc5aa03add308cc3` **native_publisher_show** — Signed by C=EE, L=Tallinn, O=Great Software Company OÜ, CN=Great Software Company OÜ. The file's hash matches the signed hash. The signature checks out against the certificate in the file. Whether Windows trusts that certificate wasn't checked. (Seen it)
- `ev_b45584c40671d7d11f2dd0914c107916873ad87c9c9cb39ab970a12e0e23082c` **native_strings_find** — 3 strings match "exact". (Seen it)
- `ev_d2bacfdf3e799f403b5d5d11fa23700f5ecd55e1611454915f1f254a4f8f0279` **native_strings_find** — 100 strings match "(Tfrm|TForm|TUninstalr|TWinControl|TVirtual|VCL)". (Seen it)
- `ev_96db8602f8c18c446a00dff35f9bc2ee59a1ab69533b67186c67a090012b2d9c` **native_strings_find** — 8 strings match "(Remove-AppxPackage|msiexec|QuietUninstallString|UninstallString|DisplayName)". (Seen it)
- `ev_b70ce7e810d97dfc0efcb5d28f7c04f6741aa473ffe14df2aee3eb789b0e79b6` **native_function_callers_list** — sub_84a170 is called from 5 places. (Worked it out)
- `ev_c7b753f6fbc9e7aec35ba3558bc0d2a8b4aacc30d2f2800ff5a84455856662a9` **native_function_callers_list** — sub_84c8d0 is called from 4 places. (Worked it out)
- `ev_210f08ccbcfb8a23a1118b510e9d378da6be459a71943419d0e1f4015fa38d5b` **native_function_callers_list** — sub_857850 is called from 1 places. (Worked it out)
- `ev_8994f7132c9f3f50f5e663496e94b6a3c090a78a4810e1eb51cb5b57a3969946` **native_rust_binary_show** — Strong Rust evidence: 228 crate(s) from Cargo paths, 66 local source path(s), symbols stripped or absent. (Seen it)
- `ev_7697ff01a0698baecc4f7c399905eafd76094686b26a24f1c90fd6ea6d7715e2` **native_strings_find** — 6 strings match "(CurrentVersion|AppxPackage|AppxManifest|Get-AppxPackage)". (Seen it)
- `ev_8b6742d3ca1046f86af311c88d2cf415aa8b3bff5967db3dd9209c4d8dc021b7` **native_strings_find** — 9 strings match "(WofCompressedData|hardlink|Hardlink|FinderNtfs|Sparse)". (Seen it)
- `ev_e45f6e158a304464ee21f3595e4b5d39c367427aafa4e25cf58716234e0e6d6a` **native_strings_find** — 36 strings match "(Leftover|leftover|Traces)". (Seen it)
- `ev_2dff3db2aff8c35aea64cb06f473312c94aa9925cec85632ac79e362499f1bb4` **native_strings_find** — 3 strings match "WaitForProcessAndChildren". (Seen it)
- `ev_69d9042acdf893d4921f1d1046e5323b3ac4ad669a269952509df336a20cb040` **native_strings_find** — 3 strings match "CustomRules". (Seen it)
- `ev_e791ae16fb775841b7a88306e17cd7fd62c6245319105d7a071d77bedb07a70a` **native_function_show** — sub_84b710 at 0x84b710: 634 bytes in 19 blocks. (Worked it out)
- `ev_70b36ac6a169ac5c31c51219d9d7ba1ff567cf771d21660498d75791604781d2` **native_function_show** — sub_84b9b0 at 0x84b9b0: 3079 bytes in 71 blocks. (Worked it out)
- `ev_9171ab0ead3d913b85b90dcd3da48ccb5e971094f426a22b3d034279e0403363` **native_function_show** — sub_1400ed6a0 at 0x1400ed6a0: 2334 bytes in 133 blocks. (Worked it out)
- `ev_5633cdd60b9ed348b2dcd2006dff79db1e648adb8df9580f8898d743dfb9ff7a` **native_function_pseudocode** — Pseudo-C for FUN_0043d212 from the Deep engine (156 lines). (Worked it out)
- `ev_42ce7043ea99e1efab37594c75fb9c6a43263403981b2d197d6197caa1e58736` **native_function_pseudocode** — Pseudo-C for FUN_0084b710 from the Deep engine (75 lines). (Worked it out)
- `ev_b586e4e77efddf06b6be060dee633eef687a9cbed3ed46488ec84e938bd79f32` **native_function_pseudocode** — Pseudo-C for FUN_1400ed6a0 from the Deep engine (404 lines). (Worked it out)
- `ev_23da5b79f8c9615ab7ba81bd7b223cb3837f1f90999a8afe8cc5cf353c5a9d15` **core_overview_show** — PE (Windows) executable for x86_64 with 7370 functions, 8229 strings, 602 imports from 20 libraries. (Worked it out)
- `ev_744038047558508da6d3dafd41fe510008b5fa60289ffc0e16427c7e8d5ed8c2` **core_overview_show** — PE (Windows) executable for x86_64 with 29342 functions, 42325 strings, 878 imports from 23 libraries. (Worked it out)
- `ev_d0e3dabbb7c2ffd5a75869e7e73d823a5db9e26d31ce537b41545b07bc9ea83b` **native_rizin_command_run** — rizin `izz~wow6432node`: 45 lines of output. (Seen it)
- `ev_98aa97f2099a2fa2e8bc297b919dcb0ebc6c576b035645a6e429c21062465588` **native_rizin_command_run** — rizin `/x b02f7d00`: 0 lines of output. (Seen it)
- `ev_e595f1a4a9933188ad5f686965f9255cc322efed3ee28f29bdb345b5817dc4c4` **native_strings_find** — 3 strings match "wow6432node". (Seen it)
- `ev_fa63a65eb21c79f64e33e609e29f5db37037011a6c68eab53592cd229e346ce8` **native_xrefs_list** — 0x277fad5 is referenced from 0 places and refers to 0. (Worked it out)
- `ev_16c5f652ece424fa471d1da6b2fc31646063c3498ec9688bd030c9154573eda5` **native_function_show** — sub_84a170 at 0x84a170: 5415 bytes in 126 blocks. (Worked it out)
- `ev_49f046f6bec3b177441667a86b38153fab4f88508814006e4b5d493a632de77c` **native_function_show** — sub_84dd30 at 0x84dd30: 2454 bytes in 86 blocks. (Worked it out)
- `ev_7a336e40184de1c08a0f66460fc5ee6437a5548ec9ee219effbe482ffed1d1ab` **native_function_pseudocode** — Pseudo-C for FUN_0084dd30 from the Deep engine (228 lines). (Worked it out)
- `ev_c3b294cbd3068c4ac846ae95504ef9b111b532ac1075286d8894338942b173a5` **native_rizin_command_run** — rizin `/x d5fa7702`: 0 lines of output. (Seen it)
- `ev_14d62a53e05dfcd87c3af6c86b4eccb9b2df040f456ceb0f3dfc02bfdf415cf0` **native_strings_browse** — 0 strings (showing 0). (Seen it)
- `ev_d283d80db48f816bc16f136f4d98e145de1c669577197e9cdf0e5fa05f4be20c` **native_bytes_show** — 256 bytes at 0x84e728. (Seen it)
- `ev_aae3ad78186b77231332f4d7932fdf7dd7c802c86a09b709ef84dcecccf6fb5f` **native_xrefs_list** — 0x69ee70 is referenced from 0 places and refers to 0. (Worked it out)
- `ev_4848d5c2a6f06353261576c2da1e5609056fcf73087170ff93c02594712fd944` **managed_assembly_show** — UninstallTools 6.3.0.0: library (dotnet-core, il-only, x64), 368 types, 2374 methods, 40 assembly references, 2 module references, targets .NETCoreApp,Version=v8.0. (Seen it)
- `ev_9fa2104333c34a85e113585f4d7546b8e7980c275fb6e0afbef89e9de05f5c39` **managed_members_list** — 2 types (2 listed from 0), 26 methods (26 with IL bodies, 326 call sites), 20 fields. (Seen it)
- `ev_bc89e5c838fec38567b3419312b9e0ee139fff5128697fef50d6895826090a45` **native_function_pseudocode** — Pseudo-C for FUN_0084a170 from the Deep engine (382 lines). (Worked it out)
- `ev_b5f1c8c7fe98cd95e953905db1b7f65538ab12f8cf89479f73b08118fe572200` **native_rizin_command_run** — rizin `/x d5fa7702 0x401000 0xE00000`: 0 lines of output. (Seen it)
- `ev_b389fbe3468a3ac595f6caba5129f5b012f1017dfda52a4658d57b576df9032b` **native_rizin_command_run** — rizin `/x 68ee2900`: 0 lines of output. (Seen it)
- `ev_a766371c3feeb7eedd4f77a80e49a653b2fd32ce5a5c9adc8f898d69f48ef730` **native_rizin_command_run** — rizin `/x d5fa7702 @ 0x1400000`: 0 lines of output. (Seen it)
- `ev_a0050e59779c1405f84b2fe4fb02f943d1ffde6defd906d5e31a9add2f5315a6` **native_rizin_command_run** — rizin `/x 68ee6900`: 2 lines of output. (Seen it)
- `ev_8bc8328d7ee5a5a53d575509d11bd10e8eedc420110c8c26e8cc7f0c4db7d7ab` **native_rizin_command_run** — rizin `/x 28f46600`: 1 lines of output. (Seen it)
- `ev_34c4b601589283bd63e1a6faef69e1b219e8e616b0ec750e11b621b67570a8af` **native_rizin_command_run** — rizin `/x 4cf46600`: 1 lines of output. (Seen it)
- `ev_3208409d1212d394ce9d100c0c08505afb6a7ab60387088d6de0232d0486a645` **native_rizin_command_run** — rizin `/x d5fa7702 @e:bin.maxstrbuf=0x2000000`: 0 lines of output. (Seen it)
- `ev_d2f69e8410941d4726186e814702b0e8f79157c327d63bdd58a3f623c8384a6c` **native_bytes_show** — 256 bytes at 0x60e2f0. (Seen it)
- `ev_c87ed8c7f74ee5e872d0bc999aff2a877a1389d3d533977c843bcef069f83589` **native_bytes_show** — 256 bytes at 0x66f444. (Seen it)
- `ev_074d89c85882810a36c2ce8007d233439e93d149b69f778550e4ef8bede5fd94` **native_functions_decompile** — Decompiled 0 of 4 functions. (Best guess)
- `ev_71288e83fc2b9f73c2aac076915552d7ebb92f6ee4e40bfdd2aea5809bd835a6` **native_function_pseudocode** — Pseudo-C for FUN_004448d9 from the Deep engine (11 lines). (Worked it out)
- `ev_8478a8f5663a3c021554a6b9222593a27e134aa7a5b78d5356e57a183c76e900` **native_function_pseudocode** — Pseudo-C for FUN_0044502e from the Deep engine (25 lines). (Worked it out)
- `ev_3886009abe058ed941d96ab0afd690528686cf7bb71eb43df40fe195144fc874` **native_bytes_show** — 256 bytes at 0x66f4a8. (Seen it)
- `ev_8ed3ef41566a2da433a1b236bfb962af4fa86c60de4a3e94865c69025c96d112` **native_bytes_show** — 256 bytes at 0x66f4f4. (Seen it)
- `ev_ba52739711afb39b1abc5f4cea187ca4ef89d87ce79a4426f536634f4e80c2e2` **native_strings_browse** — 1 strings (showing 1). (Seen it)
- `ev_c9588ac4f9d68ff885ab5da2beb4ed4dc8c5a69c916632660ff753fb0d47ad7e` **native_strings_browse** — 1 strings (showing 1). (Seen it)
- `ev_ea38df2290b9d3e0e5da1dd717afb6314b1ee31e264b028cb36f50cfc7bf1dd6` **native_strings_find** — 9 strings match "CustomRule". (Seen it)

