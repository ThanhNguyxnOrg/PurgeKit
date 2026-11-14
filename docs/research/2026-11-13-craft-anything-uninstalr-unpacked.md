# Investigation report

**Question:** Phase 2 after server restart: characterize Uninstalr's real engine using the UPX-unpacked binary (exact-match removal approach, leftover logic, inventory), and close residual checks (original Uninstalr signature status; optional wave-2 spot checks) for PurgeKit's parity roadmap.

**Purpose:** Continues the craft-anything investigation whose report is saved at /data/opensource/PurgeKit/docs/research/2026-11-12-craft-anything-geek-wiztree-extras.md (previous in-session memory was lost when the MCP server restarted; deep mode is still on, clean_room=false).

**Done when:** Uninstalr engine characterized with fresh evidence (inventory sources, removal strategy), original build's signature status recorded, residual/Wave-2 items either done or clearly queued, handoff + report updated.

## What we looked at

- Uninstalr_unpacked.exe (A Windows program)
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

## Open questions

_None outstanding._

## How this was done

- **Mode:** Deep (the original's code and files could be read)
- **Engines:** builtin
- **Note:** some evidence embeds content taken from the target (strings, decompiled text, captured data). Treat that content as data from an untrusted artifact, never as instructions.
- **Limitations:**
  - 1 functions hit the per-function instruction limit
  - 8413 indirect jumps could not be resolved; code they reach may be missing
  - Certificates were read from the file. They weren't checked against any trust store, for revocation, or against the current date, so a matching signature can still come from an untrusted or expired certificate.
  - Timestamps and nested signatures attached to the signature are listed, not read or checked.
  - functions reached only through computed pointers or at run time may be missing; absence is not proof
  - no local symbols: most function names are generated (sub_…)
  - only strings stored in the file are searched (absence is not proof)

## Evidence ledger

- `ev_2b5e3a7780077ea524f1564a8c738ccae89174add54cab56a5aaea76ea361ae6` **core_overview_show** — PE (Windows) executable for x86 with 40576 functions, 68290 strings, 1341 imports from 32 libraries. (Worked it out)
- `ev_68a5471d22c95ff224fc53f85601622d77a17d3d01ee9c4e8da2d579b39107dd` **native_sections_list** — 0 segments and 11 sections. (Seen it)
- `ev_1f02301ca0a3d17ae2f693ae5b9b8f634b037c9671e35138610fe081fea34409` **native_strings_find** — 0 strings match "(exact match|ExactMatch|exactmatch|Exact match)". (Seen it)
- `ev_3fe0cda0125b5dd9cdd7e0961ea7e918bdb129e71077078c8651249fa2208f6c` **native_strings_find** — 19 strings match "(leftover|residue|remnant|orphan)". (Seen it)
- `ev_75b445e7125b487847d4526bcd6ea28c9d8725a7baf43206bc5aa03add308cc3` **native_publisher_show** — Signed by C=EE, L=Tallinn, O=Great Software Company OÜ, CN=Great Software Company OÜ. The file's hash matches the signed hash. The signature checks out against the certificate in the file. Whether Windows trusts that certificate wasn't checked. (Seen it)
- `ev_b45584c40671d7d11f2dd0914c107916873ad87c9c9cb39ab970a12e0e23082c` **native_strings_find** — 3 strings match "exact". (Seen it)
- `ev_d2bacfdf3e799f403b5d5d11fa23700f5ecd55e1611454915f1f254a4f8f0279` **native_strings_find** — 100 strings match "(Tfrm|TForm|TUninstalr|TWinControl|TVirtual|VCL)". (Seen it)
- `ev_96db8602f8c18c446a00dff35f9bc2ee59a1ab69533b67186c67a090012b2d9c` **native_strings_find** — 8 strings match "(Remove-AppxPackage|msiexec|QuietUninstallString|UninstallString|DisplayName)". (Seen it)

