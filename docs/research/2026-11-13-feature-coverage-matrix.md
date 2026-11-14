# PurgeKit feature-coverage matrix — every feature ↔ an inheritance source (2026-11-13)

**Purpose.** For every capability PurgeKit ships (or deliberately lacks), name at least one *analyzed shipped artifact* or open-source project to inherit ideas from and develop against — for normal desktop apps, Microsoft Store apps, portable apps, and dev installs (npm/bun/node/pip/…). Sources marked **[S]** = study-only (GPL/other); everything else is behavior/architecture-level reference.

**Analyzed set (evidence in the craft-anything reports):** Geek Uninstaller 1.6 · WizTree 4.33 (+extras wave: Uninstalr, HiBit, Revo, Everything, WinDirStat 2 **[S]**, AllDup, Czkawka, BCU binary + source, WizTree32). Community dossier list is in `2026-11-13-extended-targets-and-community-triage.md`.

## 1. Inventory & Apps Manager

| PurgeKit feature | Status | Inheritance source (mechanism) | Develop next |
|---|---|---|---|
| ARP registry inventory (HKLM/HKCU × native/Wow) | Done | Geek: roots incl. `Installer\Folders`, `App Paths`; BCU source: direct+Wow hives (Wow only in 64-bit proc), `GetUninstallerType` = MSI (`WindowsInstaller`), **InnoSetup** (`"Inno Setup:"` values), **Steam** (`"Steam App "` key prefix) | Port BCU's type detection; SystemComponent toggle |
| Microsoft Store / UWP inventory | Done | Geek (MetroAppsLoader + `Remove-AppxPackage`), **Revo** ("Windows Apps" module: `remove-appxpackage` @sub_14005D9B0, AppxManifest icons, Microsoft Store link), **HiBit** (WindowsApps module + cache keys), BCU `StoreAppFactory` + StoreAppHelper.exe, Uninstalr (Appx strings) | Provisioned-package note; "Open in Store" link |
| Portable / orphaned apps | Done (LocalAppData\Programs) | BCU `DirectoryFactory` (depth ≤2, ≤40 exes, known-folder skip, metadata from best exe, `IsOrphaned`) | Extend roots + "uninstall from folder" |
| CLI global packages (npm, yarn, pnpm, bun, deno, cargo, pip, go, gem, composer, dotnet — 11) | Strong | Comparators don't cover this: BCU `ScoopFactory`/`ChocolateyFactory` (enumeration pattern), **npkill** (node_modules), **cargo-sweep**, **pip-autoremove** | Add **pipx**; Scoop/Chocolatey sweeps; verify per-manager command set |
| Toolchain versions (rustup/nvm/fnm) | Done | PurgeKit-specific; study **Volta/mise/asdf** layouts | Add volta/deno dirs |
| Game managers (Steam/Oculus/…) | **Gap** | BCU `SteamFactory` (+ orphaned games), `OculusFactory` | New module: Steam library + leftover scan |
| Windows Features / Updates | Not covered | BCU `WindowsFeatureFactory`, `WindowsUpdateFactory` | Optional integration/cleanup |
| Icons (137/138) | Done | Geek 0x42FC55 pipeline (DisplayIcon→shortcut→InstallLocation→UninstallString→MSI) + cache | — |
| Install dates | Done | Geek `tryToGetBetterInstallDate`; LoadCachedData stores TDateTime doubles | — |
| Install-location inference | Done | Geek `FindByUninstallString` + Levenshtein (FUN_00410111 DP; raw-distance threshold **39**) + "false positive" guards | Port stale-location guard exactly |
| Derived cache | Done (fingerprint) | Geek `cache.dat` v3 (gzip XML, sentinels `-`/`0xFFFFFFFF`) + LoadCachedData merge/validation | gzip for large caches |
| Duplicate-entry resolution | Done (richness score) | Geek `WhichItemBetter`: ordered flag diffs (UninstallString presence, WindowsInstaller, +0x58 flag), FullRegPath fallback; all fields logged | Adopt ordered flag comparisons |
| Uninstall execution | Better than comparators | BCU `SeparateArgsFromCommand` + raw-string fallback + NSIS `UpdateNsisStartInfo` + `safeMode`; PurgeKit rejects shell wrappers | NSIS-aware quiet params |
| Uninstaller process wait | Done (job object) | Geek `WaitForProcessAndChildren` (recursive over ProcessList, start-time/path filtered) | Optional start-time sweep |
| Silent / bulk / batch | Done | BCU `BulkUninstallTask`: concurrency limit, one loud at a time, retry, auto-kill, **simulate** | Dry-run/simulate flag |
| Force removal + reboot queue | Done | Geek force removal + `MoveFileEx` reboot strings | — |
| Leftover scoring | Done (evidence tiers) | Geek 4 scanners + **per-app rule classes** (`StopZillaCustomRule`, `WizardCustomRule`); BCU **Sift4 thresholds** (≤4 reject; ≤1 accept; publisher-trim; containment=2; cutoff len/3) + confidence levels + finders (Drive/Misc/Registry: MSI/UserData/shortcuts/firewall/COM/Prefetch/WER); Revo/Uninstalr **None/Minimal/Normal** levels + Recycle Bin | Confidence classes + per-app rule tables + rules channel |
| Quarantine / backup | Done | BCU: Recycle Bin deletion, `.reg` export before delete, restore point offer | Recycle-Bin deletion option |
| MSI handling | Done (basic) | BCU `GetMsiString` (`MsiExec /qb /X{GUID} REBOOT=ReallySuppress /norestart`); .NET Uninstall Tool list→dry-run→remove UX | Ex APIs + dry-run |

## 2. Disk Space, Duplicates, Tracking

| Feature | Status | Inheritance source | Develop next |
|---|---|---|---|
| NTFS MFT scanning | Strong | WizTree (fully mined: extents/FSCTL 0x90073, record-10 upcase, VOLDATA2 reader, **allocated = runlist clusters × cluster size**, merge dedupe, dump/CSV); WinDirStat 2 **[S]** (WOF `$WofCompressedData` async cluster-run reader, hardlink mgmt) | WOF-aware sizing in PurgeKit reader |
| Treemap & views | Done | WizTree (labels/weight/free-space/small-file grouping); WinDirStat sunburst/flame **[S]** | Optional chart modes |
| Duplicates | Done (name+size+date) | Czkawka (MIT): blake3/xxhash/crc32fast/sha2/twox-hash; image_hasher+hamming-bitwise-fast+**bk-tree**; rusty-chromaprint+symphonia+lofty; `trash`; stage = size→prefix→full hash; dupeGuru pHash **[S]**; AllDup byte-verify (not evidenced statically) | Hash pipeline (size→prefix→full) + content re-verify option |
| CSV/TSV + .pkmft + VOLDATA2 import | Done | WizTree schema (export strings verified) | split-name/max-depth options |
| Multi-root scans | Done | WizTree containment rules | Graphical multi-select picker |
| Config & deployment | Partially | WizTree: INI + **registry ConfigOverride** (mass deployment, read-only `!`), `/supportercode` etc. | Registry-based config override |
| USN journal tracker | Basic (one-buffer) | **Everything** `FUN_14004A320`: reason-mask routing, rename old-name pairing buffer, attribute-bit change filter (sparse 0x200…), parent-ref resolution + (parent,name) hash | Port reason/attr filters + rename pairing |
| Snapshots | Done | (own design) | — |

## 3. Dev tooling, cleaners, adjacent

| Feature | Status | Inheritance source | Develop next |
|---|---|---|---|
| Dev cache rules (16) | Done | BleachBit **CleanerML XML** + Winapp2.ini corpus **[S]**; Dism++ `Data.xml` | winapp2.ini importer (gated) |
| VS Code / JetBrains extensions | Done | (own) | — |
| Project sweeper (node_modules, target, venv…) | Done | **npkill** (live-updating node_modules scan, multi-select, safe delete; TS/OSS) | npkill-style live results |
| WSL shrinker | Done | WSLCompact (no-admin export/import) **[S]**; `wsl --set-sparse`; Optimize-VHD | Export/import fallback without admin |
| PATH cleaner | Done | Rapid Environment Editor (study target) | — |
| Startup manager | Done | Sysinternals Autoruns (breadth, autorunsc export), Revo Run-key surface | Cross-check vs autorunsc |
| COM / shell cleanup | Done (COM) | BCU finders; **Context Menu Manager** (Blocked-key disabling instead of delete) **[S]**; ShellExView (Blocked list, `/external`) | Non-destructive "block" mode |
| Locked files | Done (Restart Manager) | PowerToys **FileLocksmith** (`--json`/`--kill`/`--wait`) | CLI-mode inspiration |
| Driver store (new) | Not covered | **RAPR** (DriverStore Explorer) **[S]** | Optional module |
| Shell menus (new) | Not covered | ContextMenuManager / ShellExView **[S]** | Optional module |

## 4. Dev installs deep-dive (npm/bun/node/pip/…)

| Manager | PurgeKit today | Enumeration source of truth | Inheritance source / next |
|---|---|---|---|
| npm / yarn / pnpm / **bun** / **deno** | In `cli_dev.rs` (+ inventory/dev_rules) | each manager's own CLI (`ls -g`, `bun pm ls`, `deno info`, …) | npkill for node_modules parity |
| pip | In `cli_dev.rs` | `pip list` family | **pip-autoremove**; **pipx** not covered → add `pipx list --json` |
| cargo | In `cli_dev.rs` | `cargo install --list` | **cargo-sweep** for `~/.cargo` caches |
| go | In `cli_dev.rs` | `GOPATH/bin` scan | `go clean -modcache` option |
| gem / composer / dotnet | In `cli_dev.rs` | manager CLIs | — |
| Scoop / Chocolatey | Only as classifier keywords | BCU `ScoopFactory` / `ChocolateyFactory` (source) | Add package sweeps |
| Volta/other toolchains | Not covered | Volta/mise layouts | Optional |

## 5. Cross-cutting

| Area | Inheritance source | Develop next |
|---|---|---|
| CLI | BCU-console (unattended commands), Uninstalr corporate unattended | CLI parity (`cache prune`, `wsl`, `export --json`) |
| i18n | Geek (43 XML language resources = UI contract) | Enable the Vietnamese dictionary |
| Trust & signing | Uninstalr signed by *Great Software Company OÜ*; HiBit portable **unsigned**; WizTree EV | Catalog-aware signature checks; sign releases |
| Packaging | **Uninstalr ships UPX-packed** (19% ratio) — lesson: breaks signing; AV flags | Keep MSI/NSIS, don't pack after signing |
| Update check | WizTree/Geek check-for-updates patterns | Optional updater |
| Rules channel | BleachBit + Winapp2.ini **[S]** | Signed rule feed |

## 6. Verdict

- **Normal apps:** fully covered (Geek/BCU/Revo/HiBit/Uninstalr + own implementation).
- **Microsoft Store apps:** fully covered — Geek, Revo (Windows Apps module, `remove-appxpackage`), HiBit (WindowsApps cache), BCU, Uninstalr, and PurgeKit's `uwp.rs`.
- **Portable apps:** covered via BCU DirectoryFactory.
- **Dev installs:** strong on 11 managers + 3 toolchains; remaining: pipx, Scoop/Chocolatey sweeps, game launchers (Steam/Oculus) — sources named above.
- Everything else in the matrix has a named inheritance source; the only intended “no sources” are study-only **GPL** projects, which are explicitly not copyable.

## Wave 3 (added 2026-11-13, later the same day) — next-best apps

All downloaded to `targets/extras2/` (outside the repo) and triaged with craft-anything (evidence in `2026-11-13-craft-anything-continuation.md` and the session journal).

| App | License | What it adds / verifies | Next step for PurgeKit |
|---|---|---|---|
| **Sysinternals Autoruns 64** | Freeware (study-ok) | ASEP breadth: Boot Execute, WinLogon (+Wow6432Node), **Active Setup\Installed Components**, AppInit_DLLs strings + scan functions | Extend `autoruns.rs` with these four sources |
| **RAPR 1.0.75** (.NET 4.7.2) | GPL-2.0 **[S]** | Driver-store module reference; managed reader OK (163 types); no pnputil/setupapi strings (bounded) | Driver-store module skeleton (SetupAPI enumeration) |
| **ContextMenuManager 3.3.3.1** (.NET 4.0) | GPL-3.0 **[S]** | Shell-menu module reference (319 types); non-destructive **Blocked-key** technique (README) | Shell-menu module with block-mode |
| **RegistryChangesView** (NirSoft) | Freeware | Snapshot design: CreateSnapshot/Compare/RegistrySnapshots, CLI `/CreateSnapshot`, `/CreateSecuritySnapshot`, temp snapshots | Snapshot CLI + security snapshot + temp capture |
| **fclones 0.35** | MIT | Dup pipeline: blake3/sha2/xxhash-rust, **sled checkpoint cache**, **reflink dedupe**, rayon, 4,000 symbols present | Hash pipeline + cached index + reflink/hardlink dedupe |
| **WizFile 3.15** | Free personal | MFT scan + `.MFT` file filter (same house as WizTree) | Cross-check for search module |
| **TreeSize Free** | Free (private) | MFT-when-elevated (vendor docs); probe bounded (localized strings) | Enterprise reference only |
| **Dism++** | Freeware (repo MIT) | `Data.xml` rules (loaded at sub_1400A71D8); `$HardLinkBackup` / `DismHardLinkMerge` WinSxS tricks | Rules-channel + WinSxS backup technique |
| **PrivaZer** | Freeware (closed) | USN residual detection; allocated/slack-space overwrite; multi-pass erase | Study-only reference for secure-delete options |
| **SpaceSniffer** | Freeware | Live treemap UX reference | UI polish reference |
| **dupeGuru 4.3.1** | GPL-3.0 **[S]** | Single-file packed (reader unrecognized) → source-study only | pHash reference from source |
| **npkill 1.0.0 source** | MIT | node_modules cleaner layout (`lib/core/services/{files,targets}`) | Live-scan UX parity for Project Sweeper |

With wave 3, every gap listed in section 6 now has a concrete, analyzed source; the remaining work is implementation, not discovery.
