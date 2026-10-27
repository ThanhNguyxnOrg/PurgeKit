<script lang="ts">
  import { onMount } from "svelte";
  import { Shield, HardDrive, Info, Heart, RefreshCw, Bug, ExternalLink, CheckCircle, Trash2, Lightbulb, Database, Sun, Moon, Monitor } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { toast } from "../toast.svelte";
  import { theme } from "../theme.svelte";

  let scanLevel = $state("safe");
  let autoBackup = $state(true);
  let createRestorePoint = $state(true);
  let forceUnlock = $state(false);
  let isAdmin = $state(false);
  let projectScanPaths = $state<string[]>([]);

  let isCheckingUpdates = $state(false);
  let appVersion = $state("");
  let updateMessage = $state<{ type: "success" | "error"; text: string } | null>(null);

  interface QuarantineItem {
    id: string;
    name: string;
    original_path: string;
    quarantine_path: string;
    created_at: string;
  }

  interface PendingRebootItem {
    path: string;
    source: string;
  }

  let quarantineItems = $state<QuarantineItem[]>([]);
  let isLoadingQuarantine = $state(false);
  let pendingRebootItems = $state<PendingRebootItem[]>([]);
  let isLoadingPendingReboot = $state(false);

  interface DerivedCacheSummary {
    entries: number;
    bytes: number;
  }

  let cacheSummary = $state<DerivedCacheSummary | null>(null);
  let isClearingCache = $state(false);

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
  }

  async function loadPendingReboot() {
    isLoadingPendingReboot = true;
    try {
      pendingRebootItems = await invoke<PendingRebootItem[]>("get_pending_reboot_items");
    } catch (e) {
      console.error("Failed to load pending reboot items:", e);
    } finally {
      isLoadingPendingReboot = false;
    }
  }

  async function loadCacheSummary() {
    try {
      cacheSummary = await invoke<DerivedCacheSummary>("get_derived_cache_summary");
    } catch (e) {
      console.error("Failed to load app cache summary:", e);
    }
  }

  async function clearDerivedCache() {
    if (!confirm("Clear cached install locations, icons and sizes? The next app refresh will rebuild them.")) {
      return;
    }
    isClearingCache = true;
    try {
      const removed = await invoke<number>("clear_derived_cache");
      toast.show(`Cleared ${removed.toLocaleString()} cached app records.`, "success");
      await loadCacheSummary();
    } catch (e: any) {
      toast.show(`Could not clear the app cache: ${e.toString()}`, "error");
    } finally {
      isClearingCache = false;
    }
  }

  async function loadAppVersion() {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "";
    }
  }

  async function loadQuarantine() {
    isLoadingQuarantine = true;
    try {
      quarantineItems = await invoke<QuarantineItem[]>("list_quarantine_items");
    } catch (e) {
      console.error("Failed to load quarantine items:", e);
    } finally {
      isLoadingQuarantine = false;
    }
  }

  async function restoreQuarantine(id: string) {
    try {
      await invoke("restore_quarantine_item", { id });
      toast.show("File restored from quarantine successfully!", "success");
      await loadQuarantine();
    } catch (e: any) {
      toast.show(`Restoration failed: ${e.toString()}`, "error");
    }
  }

  async function deleteQuarantine(id: string) {
    if (!confirm("Are you sure you want to permanently delete this file? This action cannot be undone.")) {
      return;
    }
    try {
      await invoke("delete_quarantine_item", { id });
      toast.show("File deleted permanently successfully!", "success");
      await loadQuarantine();
    } catch (e: any) {
      toast.show(`Failed to delete: ${e.toString()}`, "error");
    }
  }

  async function loadSettings() {
    try {
      const settings = await invoke<{
        enable_undocumented_force_unlock: boolean;
        scan_level: string;
        backup_before_delete: boolean;
        create_restore_point: boolean;
        project_scan_paths: string[];
      }>("get_settings");
      
      scanLevel = settings.scan_level;
      autoBackup = settings.backup_before_delete;
      createRestorePoint = settings.create_restore_point;
      forceUnlock = settings.enable_undocumented_force_unlock;
      projectScanPaths = settings.project_scan_paths || [];
      
      isAdmin = await invoke<boolean>("check_is_admin");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function handleSave() {
    try {
      await invoke("save_settings", {
        settings: {
          enable_undocumented_force_unlock: forceUnlock,
          scan_level: scanLevel,
          backup_before_delete: autoBackup,
          create_restore_point: createRestorePoint,
          project_scan_paths: projectScanPaths
        }
      });
      toast.show("Settings configuration saved successfully!", "success");
    } catch (e: any) {
      toast.show(`Error saving settings: ${e.toString()}`, "error");
    }
  }

  async function restartAsAdmin() {
    if (!confirm("Restart PurgeKit with Administrator privileges? The current window will close.")) {
      return;
    }
    try {
      await invoke("restart_as_admin");
    } catch (e: any) {
      toast.show(`Could not restart as Administrator: ${e.toString()}`, "error");
    }
  }

  async function checkUpdates() {
    isCheckingUpdates = true;
    updateMessage = null;
    try {
      appVersion = await getVersion();
      updateMessage = {
        type: "success",
        text: `Installed version: PurgeKit v${appVersion}. Automatic update checks are not configured yet.`
      };
    } catch (e: any) {
      updateMessage = { type: "error", text: `Could not read the installed version: ${e.toString()}` };
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function reportBug() {
    try {
      const osName = "Windows";
      const isAdminStr = isAdmin ? "Yes" : "No";
      const versionLabel = appVersion ? `v${appVersion}` : "unknown";
      
      const title = encodeURIComponent("[Bug] ");
      const body = encodeURIComponent(
        `**Describe the bug**\n` +
        `A clear and concise description of what the bug is.\n\n` +
        `**To Reproduce**\n` +
        `Steps to reproduce the behavior:\n` +
        `1. Go to '...'\n` +
        `2. Click on '...'\n` +
        `3. See error\n\n` +
        `**Expected behavior**\n` +
        `A clear and concise description of what you expected to happen.\n\n` +
        `**System Environment Information:**\n` +
        `- OS: ${osName}\n` +
        `- PurgeKit Version: ${versionLabel}\n` +
        `- Running as Administrator: ${isAdminStr}\n\n` +
        `**Additional context**\n` +
        `Add any other context or logs here.`
      );
      
      const issueUrl = `https://github.com/ThanhNguyxnOrg/PurgeKit/issues/new?title=${title}&body=${body}&labels=bug`;
      await openUrl(issueUrl);
    } catch (e) {
      console.error(e);
      toast.show("Failed to open browser.", "error");
    }
  }

  async function requestFeature() {
    try {
      const title = encodeURIComponent("[Feature] ");
      const body = encodeURIComponent(
        `**Is your feature request related to a problem? Please describe.**\n` +
        `A clear and concise description of what the problem is. Ex. I'm always frustrated when [...]\n\n` +
        `**Describe the solution you'd like**\n` +
        `A clear and concise description of what you want to happen.\n\n` +
        `**Describe alternatives you've considered**\n` +
        `A clear and concise description of any alternative solutions or features you've considered.\n\n` +
        `**Additional context**\n` +
        `Add any other context or screenshots about the feature request here.`
      );
      
      const issueUrl = `https://github.com/ThanhNguyxnOrg/PurgeKit/issues/new?title=${title}&body=${body}&labels=enhancement`;
      await openUrl(issueUrl);
    } catch (e) {
      console.error(e);
      toast.show("Failed to open browser.", "error");
    }
  }

  onMount(() => {
    loadSettings();
    loadQuarantine();
    loadPendingReboot();
    loadAppVersion();
    loadCacheSummary();
  });
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        Settings
      </h2>
      <p class="text-xs text-text-muted mt-1">Configure scanning behaviors, elevation preferences and clean policies</p>
    </div>
  </header>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-6 max-w-4xl space-y-6">
    <!-- Appearance -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
        <Sun class="w-4 h-4 text-accent" />
        Appearance
      </h3>
      <p class="text-xs text-text-muted font-sans leading-relaxed">
        Choose a palette or follow the Windows app theme. Currently showing the
        <span class="font-mono text-text-secondary">{theme.resolved}</span> palette.
      </p>
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-2" role="group" aria-label="Color theme">
        <button
          onclick={() => theme.setMode("auto")}
          aria-pressed={theme.mode === "auto"}
          class="flex items-start gap-3 p-3 rounded-lg border text-left transition-colors {theme.mode === 'auto' ? 'border-accent bg-accent-subtle text-accent' : 'border-border-default bg-app-bg text-text-secondary hover:bg-elevated-bg'}"
        >
          <Monitor class="w-4 h-4 mt-0.5 shrink-0" />
          <span class="flex flex-col">
            <span class="text-xs font-bold">Auto</span>
            <span class="text-[10px] text-text-muted mt-0.5">Follow Windows</span>
          </span>
        </button>
        <button
          onclick={() => theme.setMode("light")}
          aria-pressed={theme.mode === "light"}
          class="flex items-start gap-3 p-3 rounded-lg border text-left transition-colors {theme.mode === 'light' ? 'border-accent bg-accent-subtle text-accent' : 'border-border-default bg-app-bg text-text-secondary hover:bg-elevated-bg'}"
        >
          <Sun class="w-4 h-4 mt-0.5 shrink-0" />
          <span class="flex flex-col">
            <span class="text-xs font-bold">Light</span>
            <span class="text-[10px] text-text-muted mt-0.5">Warm paper palette</span>
          </span>
        </button>
        <button
          onclick={() => theme.setMode("dark")}
          aria-pressed={theme.mode === "dark"}
          class="flex items-start gap-3 p-3 rounded-lg border text-left transition-colors {theme.mode === 'dark' ? 'border-accent bg-accent-subtle text-accent' : 'border-border-default bg-app-bg text-text-secondary hover:bg-elevated-bg'}"
        >
          <Moon class="w-4 h-4 mt-0.5 shrink-0" />
          <span class="flex flex-col">
            <span class="text-xs font-bold">Dark</span>
            <span class="text-[10px] text-text-muted mt-0.5">Low-light palette</span>
          </span>
        </button>
      </div>
      <label class="flex items-center justify-between gap-4 p-3 rounded-lg bg-app-bg border border-border-default cursor-pointer">
        <span class="flex flex-col">
          <span class="text-xs font-bold text-text-primary">Color-blind friendly palette</span>
          <span class="text-[10px] text-text-muted mt-0.5">
            Swaps chart and status colors to a blue/orange/magenta-safe set.
          </span>
        </span>
        <input
          type="checkbox"
          checked={theme.colorBlind}
          onchange={(event) => theme.setColorBlind(event.currentTarget.checked)}
          class="w-4 h-4 rounded accent-accent bg-app-bg border-border-default focus:ring-accent/30 cursor-pointer shrink-0"
        />
      </label>
    </section>

    <!-- Scanner settings -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
        <HardDrive class="w-4 h-4 text-accent" />
        Scanning Preferences
      </h3>

      <div class="space-y-4">
        <!-- Scan level -->
        <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 py-3 border-b border-border-default">
          <div>
            <label for="scan-level" class="text-sm font-medium text-text-secondary">Scanner Remnants Level</label>
            <span class="block text-xs text-text-muted mt-0.5">Determine how aggressive the scanner search for remnant registry keys and file folders.</span>
          </div>
          <select
            id="scan-level"
            bind:value={scanLevel}
            class="px-3 py-1.5 rounded-lg bg-app-bg border border-border-default text-xs text-text-primary outline-none focus:border-accent/50 cursor-pointer"
          >
            <option value="safe">Safe (Only obvious paths)</option>
            <option value="moderate">Moderate (Standard match)</option>
            <option value="aggressive">Aggressive (Deep search - review required)</option>
          </select>
        </div>

        <!-- Auto backup -->
        <div class="flex items-center justify-between py-3 border-b border-border-default">
          <div>
            <label for="auto-backup" class="text-sm font-medium text-text-secondary">Auto Backup Registry</label>
            <span class="block text-xs text-text-muted mt-0.5">Automatically backup registry keys in .reg files before purging.</span>
          </div>
          <input
            type="checkbox"
            id="auto-backup"
            bind:checked={autoBackup}
            class="w-4 h-4 rounded accent-accent bg-app-bg border-border-default focus:ring-accent/30 cursor-pointer"
          />
        </div>

        <!-- System Restore Point -->
        <div class="flex items-center justify-between py-3 border-b border-border-default">
          <div>
            <label for="create-restore-point" class="text-sm font-medium text-text-secondary">Create System Restore Point</label>
            <span class="block text-xs text-text-muted mt-0.5">Create a Windows System Restore Point before purging remnants (requires Admin).</span>
          </div>
          <input
            type="checkbox"
            id="create-restore-point"
            bind:checked={createRestorePoint}
            class="w-4 h-4 rounded accent-accent bg-app-bg border-border-default focus:ring-accent/30 cursor-pointer"
          />
        </div>

        <!-- Hidden setting: force unlock -->
        <div class="flex items-center justify-between py-3">
          <div>
            <label for="force-unlock" class="text-sm font-medium text-text-secondary">Force Unlock Files</label>
            <span class="block text-xs text-text-muted mt-0.5">Enable undocumented force handle duplication to unlock locked remnants (Phase 4).</span>
          </div>
          <input
            type="checkbox"
            id="force-unlock"
            bind:checked={forceUnlock}
            class="w-4 h-4 rounded accent-accent bg-app-bg border-border-default focus:ring-accent/30 cursor-pointer"
          />
        </div>
      </div>
    </section>

    <!-- Permissions settings -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
        <Shield class="w-4 h-4 text-accent" />
        Permissions & Elevation
      </h3>

      {#if isAdmin}
        <div class="flex items-center gap-3 p-3.5 rounded-lg bg-success/10 border border-success/20 mb-4 animate-fade-in">
          <Shield class="w-5 h-5 text-success" />
          <div class="flex flex-col">
            <span class="text-xs font-bold text-text-primary">Privileged Mode Active</span>
            <span class="text-[10px] text-text-muted font-mono">Running as Administrator (Standard Admin)</span>
          </div>
        </div>
      {:else}
        <div class="flex items-center gap-3 p-3.5 rounded-lg bg-danger/10 border border-danger/20 mb-4 animate-fade-in">
          <Shield class="w-5 h-5 text-danger" />
          <div class="flex flex-col flex-1">
            <span class="text-xs font-bold text-text-primary">Standard Mode Active</span>
            <span class="text-[10px] text-text-muted font-mono">Running as standard user. Some clean features will be restricted.</span>
          </div>
          <button
            onclick={restartAsAdmin}
            class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-accent hover:bg-accent-hover text-white text-[11px] font-semibold transition-colors"
          >
            <RefreshCw class="w-3.5 h-3.5" />
            Restart as Administrator
          </button>
        </div>
      {/if}
    </section>

    <!-- App inventory cache -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
        <Database class="w-4 h-4 text-accent" />
        App Inventory Cache
      </h3>
      <p class="text-xs text-text-muted font-sans leading-relaxed">
        Caches resolved install locations, icons and sizes so refreshing the app list skips
        expensive folder scans. Registry changes invalidate entries, and cached paths are
        checked again before use.
      </p>
      <div class="flex items-center justify-between gap-3 p-3.5 rounded-lg bg-app-bg border border-border-default">
        <div class="flex flex-col">
          <span class="text-xs font-bold text-text-primary">
            {cacheSummary ? `${cacheSummary.entries.toLocaleString()} apps cached` : "Cache not loaded"}
          </span>
          <span class="text-[10px] text-text-muted font-mono mt-0.5">
            {cacheSummary ? `${formatBytes(cacheSummary.bytes)} on disk` : "—"}
          </span>
        </div>
        <button
          onclick={clearDerivedCache}
          disabled={isClearingCache || !cacheSummary || cacheSummary.entries === 0}
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-danger/10 hover:bg-danger text-danger hover:text-white border border-danger/20 text-[11px] font-semibold transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <Trash2 class="w-3.5 h-3.5" />
          {isClearingCache ? "Clearing…" : "Clear cache"}
        </button>
      </div>
    </section>

    <!-- Quarantine Manager Section -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
          <svg class="w-4 h-4 text-accent animate-pulse" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            <line x1="9" y1="9" x2="15" y2="15"/>
            <line x1="15" y1="9" x2="9" y2="15"/>
          </svg>
          Quarantine Manager
        </h3>
        <button
          onclick={loadQuarantine}
          disabled={isLoadingQuarantine}
          class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg transition-colors cursor-pointer"
        >
          <RefreshCw class="w-3.5 h-3.5 text-text-secondary {isLoadingQuarantine ? 'animate-spin text-accent' : ''}" />
        </button>
      </div>

      {#if quarantineItems.length === 0}
        <div class="py-6 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/10 text-xs text-text-muted">
          No files in quarantine.
        </div>
      {:else}
        <div class="space-y-3 max-h-[300px] overflow-y-auto pr-1">
          {#each quarantineItems as item}
            <div class="flex items-center justify-between p-3 rounded-lg bg-app-bg border border-border-default text-xs gap-3">
              <div class="flex flex-col min-w-0 flex-1">
                <span class="font-bold text-text-primary truncate">{item.name}</span>
                <span class="text-[10px] text-text-muted truncate mt-0.5" title={item.original_path}>{item.original_path}</span>
                <span class="text-[9px] text-accent/70 font-mono mt-0.5">{item.created_at}</span>
              </div>
              <div class="flex items-center gap-2">
                <button
                  onclick={() => restoreQuarantine(item.id)}
                  class="px-2.5 py-1.5 rounded-lg bg-accent/10 hover:bg-accent hover:text-white text-accent border border-accent/20 transition-all font-semibold active:scale-95 cursor-pointer text-[11px]"
                >
                  Restore
                </button>
                <button
                  onclick={() => deleteQuarantine(item.id)}
                  class="p-1.5 rounded-lg bg-danger/10 hover:bg-danger hover:text-white text-danger border border-danger/20 transition-all active:scale-95 cursor-pointer"
                  title="Delete permanently"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Pending Reboot Section -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
          <RefreshCw class="w-4 h-4 text-warning" />
          Pending Reboot
        </h3>
        <button
          onclick={loadPendingReboot}
          disabled={isLoadingPendingReboot}
          class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg transition-colors cursor-pointer"
          aria-label="Refresh pending reboot items"
        >
          <RefreshCw class="w-3.5 h-3.5 text-text-secondary {isLoadingPendingReboot ? 'animate-spin text-accent' : ''}" />
        </button>
      </div>

      <p class="text-xs text-text-muted font-sans leading-relaxed">
        Paths queued for deletion by Windows at the next reboot. They still occupy disk space until the reboot happens.
      </p>

      {#if pendingRebootItems.length === 0}
        <div class="py-5 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/10 text-xs text-text-muted">
          No pending reboot deletions.
        </div>
      {:else}
        <div class="space-y-2 max-h-[220px] overflow-y-auto pr-1">
          {#each pendingRebootItems as item}
            <div class="p-3 rounded-lg bg-app-bg border border-border-default text-xs">
              <span class="block font-mono text-text-primary break-all">{item.path}</span>
              <span class="block text-[9px] text-text-muted font-mono mt-1">{item.source}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- About Section -->
    <section class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
      <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
        <Info class="w-4 h-4 text-text-muted" />
        About PurgeKit
      </h3>

      <div class="flex flex-col md:flex-row gap-6 items-center md:items-start text-center md:text-left">
        <div class="w-16 h-16 rounded-lg bg-app-bg border border-border-default flex items-center justify-center shadow-sm flex-shrink-0">
          <img src="/logo.png" alt="PurgeKit Logo" class="w-10 h-10 object-contain animate-fade-in" />
        </div>
        
        <div class="flex-1 space-y-2">
          <h4 class="text-base font-bold text-text-primary">PurgeKit Ultimate Uninstaller</h4>
          <p class="text-xs text-text-muted font-mono">Version {appVersion || "unknown"}</p>
          <p class="text-xs text-text-secondary leading-relaxed font-sans max-w-xl">
            A free, open-source tool developed to purge stubborn remnants and developer tool caches. Under Apache License 2.0.
          </p>
          <div class="flex items-center justify-center md:justify-start gap-1 text-[11px] text-text-muted">
            <span>Made with</span>
            <Heart class="w-3.5 h-3.5 text-danger fill-danger inline" />
            <span>using Svelte 5 & Rust (Tauri v2).</span>
          </div>

          <div class="flex flex-wrap gap-2.5 pt-3 justify-center md:justify-start">
            <button
              onclick={checkUpdates}
              disabled={isCheckingUpdates}
              class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow cursor-pointer"
            >
              <RefreshCw class="w-3.5 h-3.5 {isCheckingUpdates ? 'animate-spin' : ''}" />
              {isCheckingUpdates ? 'Reading...' : 'Version'}
            </button>
            
            <button
              onclick={reportBug}
              class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 transition-all text-text-primary cursor-pointer"
            >
              <Bug class="w-3.5 h-3.5 text-danger/80" />
              Report Bug
              <ExternalLink class="w-3 h-3 text-text-muted" />
            </button>

            <button
              onclick={requestFeature}
              class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 transition-all text-text-primary cursor-pointer"
            >
              <Lightbulb class="w-3.5 h-3.5 text-accent" />
              Suggest Feature
              <ExternalLink class="w-3 h-3 text-text-muted" />
            </button>
          </div>

          {#if updateMessage}
            <div class="mt-3 p-3 rounded-lg flex items-center gap-2 border animate-fade-in bg-success/10 text-success border-success/20">
              <CheckCircle class="w-4 h-4 flex-shrink-0 text-success" />
              <span class="text-xs font-sans text-success">{updateMessage.text}</span>
            </div>
          {/if}
        </div>
      </div>
    </section>

    <!-- Save Button -->
    <div class="flex justify-end pt-4">
      <button
        onclick={handleSave}
        class="px-5 py-2.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white active:scale-95 transition-all shadow cursor-pointer"
      >
        Save Changes
      </button>
    </div>
  </div>
</div>

