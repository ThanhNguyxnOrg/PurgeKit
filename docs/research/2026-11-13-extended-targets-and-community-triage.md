# Extended targets and community triage — 2026-11-13

**Scope:** community-ranked candidates in PurgeKit's space (uninstallers, disk analyzers, cleaners/adjacent), downloaded hash-verified for static study with craft-anything; complements the REA docs and `docs/research/2026-11-1{2,3}-craft-anything-*.md` reports. **Policy:** GPL apps are study-only; no code copying. Binaries live outside the repository.

## 1. Uninstaller / deep-removal candidates

| App | License / price | Free / portable build | Verdict (community) | Notes |
|---|---|---|---|---|
| Bulk Crap Uninstaller 6.3 | Apache-2.0 | GitHub releases (portable .7z) | Respected FOSS, long-time #1 pick; UI dated | Best studied as source (cloned locally); .NET 8; RegistryFactory / JunkManager (Sift4 confidence) |
| Revo Uninstaller Free | Free / Pro ≈ $25/yr | portable zip | Respected, "very safe"; free tier misses forced uninstall, monitoring, batch | Strings: leftovers contract + Recycle Bin deletion + registry surface incl. SharedDlls, Printers, RegisteredApplications |
| HiBit Uninstaller 4.0.10 | Free (donation) | portable zip (official hibitsoft.ir only) | Modern favorite; copycat domains exist (hibit-uninstaller.com flagged) | Official portable has **no Authenticode signature** |
| Uninstalr | Freeware (optional Pro) | portable exe | Novel "exact-match" engine, viral self-published benchmarks; caveat: self-benchmarked | **UPX-packed** (needs `upx -d`); Delphi/VCL; leftovers None/Minimal/Normal UI; validly signed by Great Software Company OÜ (EE) |
| IObit Uninstaller Free | Free / Pro | installer | Warned against: Malwarebytes PUP.IObit detection; 2009 Malwarebytes DB theft; bundling history | Study monetization/PUP flows only |
| Glary Absolute Uninstaller | Free | installer | Brand stigma: Microsoft PUA:Win32/GlaryUtilities on the suite | Low priority |
| Wise Program Uninstaller | Free | installer | "Competent" but brand PUP.Optional.WiseCare stigma | Low priority |
| UninstallView (NirSoft) | Freeware | x64 zip | Sysadmin standard; data-extraction reference only (multi-hive/remote/offline) | AV false positives are documented vendor behavior |
| Ashampoo UnInstaller 15 Free | Giveaways | vendor page (bot-403) | Rarely the community pick; email/key funnels | Med priority |
| Total Uninstall 7.6 | Trial / $24.95+ | setup (trial) | Legacy respect ("removes all traces"); ranked poorly in 2023 benchmark; no free tier | Med priority |
| ZSoft Uninstaller | Freeware (dormant 2008) | zsoft.dk | Historical: first popular before/after snapshot | Low priority |

## 2. Disk analyzers / MFT / search / duplicates

| App | License / price | Free / portable | Verdict | Notes |
|---|---|---|---|---|
| WizTree 4.33 | Free personal; commercial paid | portable zip | The r/sysadmin default; free tier excludes commercial use | MFT engine verified with craft-anything (extents, record #10 upcase, VOLDATA2 reader; merge stage dedupes by reference) |
| Everything 1.4/1.5 | Freeware | x64 zip | Universally praised; 1.5 beta | USN journal + MFT indexing confirmed by strings (`create-usn-journal`, `_db_monitor_ntfs_process_usn_records`) |
| WinDirStat 2 | GPL-3.0 | portable zip | Rewrite received well ("as fast as WizTree") | Hardlink management + `$WofCompressedData` cluster-run reader confirmed in binary (study-only) |
| TreeSize Free | Free (private use); Personal €20.4/yr | portable zip | Enterprise-respected; free gated (no servers, telemetry opt-out) | MFT scan when elevated (vendor changelog) |
| Czkawka 12.0.2 | MIT (core) / GPL-3.0 (Krokiet GUI) | release zips | FOSS favorite | 228 crates: blake3/xxhash-rust/crc32fast/sha2; image_hasher + hamming-bitwise-fast + bk-tree; rusty-chromaprint + symphonia + lofty; trash; sevenz-rust2/zip/ruzstd; rayon |
| AllDup 4.5 | Freeware | portable zip | DataHoarder favorite; byte-by-byte verification stage | Download kept for later study |
| dupeGuru 4.3.1 | GPL-3.0 | win64 exe | Trusted but ageing (last release 2022) | Med priority |
| SpaceSniffer | Freeware | x64 zip | Treemap UX darling; directory-walk not MFT | Quietly alive (2026 build) |
| UltraSearch / WizFile / Disk Savvy / Scanner | mixed | — | niche / paid / historical | Low priority |

## 3. Cleaners and adjacent maintenance

| App | License | Verdict | Notes |
|---|---|---|---|
| BleachBit 6.0.6 | GPL-3.0 | Ethical baseline; CleanerML XML rules + Winapp2.ini ingestion | Study rule format only |
| Dism++ 10.1 | Freeware (repo MIT) | Loved but binary frozen (Mar 2023) | `Data.xml` external rule file idea |
| PrivaZer | Freeware | Well-reviewed but closed | Free-space overwrite, artifact cleanup |
| CCleaner | Freemium | Trust-scarred: 2017 Floxif backdoor (2.27M installs), 2018 Avast bundling, 2020 PUA flag | Study winapp2.ini rule format only |
| DriverStore Explorer (RAPR) 1.0.75 | GPL-2.0 | De-facto driver-store manager, active | — |
| Autoruns (Sysinternals) | Freeware | Gold standard | Autorunsc CSV/XML export, signed verification |
| Context Menu Manager 3.3.3.1 | GPL-3.0 | Popular (20.4k★), semi-dormant | "Blocked"-key technique instead of delete |
| ShellExView 2.01 (NirSoft) | Freeware | Classic, frozen 2019 | Disk Cleanup handler COM objects; `/external` offline scans |
| WSLCompact 8.7 | GPL-3.0 | Liked niche tool | Unprivileged export/import compaction |

## 4. Artifacts downloaded (outside the repo, `targets/`)

`/data/opensource/targets/` — `geek/geek.exe`, `wiztree/{WizTree.exe,WizTree64.exe}`, `extras/{uninstalr,hibit,revo,everything,windirstat,alldup,czkawka,bcu}` (+ `bcu-src` shallow clone, `tools/upx` 5.3.0). All sha256 recorded in the 2026-11-13 craft-anything report appendix and the session handoff.

## 5. craft-anything wave-2 outcomes (see 2026-11-13 reports for evidence ids)

- **WinDirStat 2:** hardlink-management strings (`ShowCreateHardlinkPrompt`, `ProcessHardlinks`, `CreateHardLinkW`) + async cluster-run reader referencing `$WofCompressedData` (4 MB aligned buffers, overlapped I/O).
- **Revo:** registry surface survey (SharedDlls refcounts, Run/RunOnce/RunOnceEx incl. Wow6432Node, Print\Printers, RegisteredApplications, Clients, WOW6432Node Uninstall).
- **Czkawka:** full crate inventory (see table).
- **BCU:** engine assembly `UninstallTools.dll` verified (.NET); source cloned for source-assisted study.
- **Uninstalr:** UPX unpack mandatory; Delphi/VCL; leftovers None/Minimal/Normal; ARP WoW64 + Appx strings; valid signature (Great Software Company OÜ). Function-level matcher closed as bounded (tooling limits: no builtin xrefs for its strings; rizin skips regions >10 MB).
- **WizTree:** merge stage is an in-place reference-keyed dedupe (fields merged, flags OR-ed, no duplicate rows) → hardlink-counted-once reading supported; sparse/compressed accounting still open.
- **Geek:** `WaitForProcessAndChildren` recursion verified (REA caveat resolved); `CScannerCustomRules` rule source still open (RTTI xref empty).

## 6. License traps to keep visible

- **GPL-3.0 — study, never copy:** WinDirStat 2, BleachBit, dupeGuru, Context Menu Manager, Krokiet GUI.
- **Copyable:** BCU (Apache-2.0), Czkawka core/MFT lib (MIT), Winapp2.ini (community rules corpus).
- Free tiers exclude commercial use: WizTree, UltraSearch, TreeSize (servers).
