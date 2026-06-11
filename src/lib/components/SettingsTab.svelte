<script lang="ts">
  import { onMount } from "svelte";
  import { Shield, HardDrive, Info, Heart, RefreshCw, Bug, ExternalLink, CheckCircle } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "../toast";

  let scanLevel = $state("safe");
  let autoBackup = $state(true);
  let forceUnlock = $state(false);
  let isAdmin = $state(false);

  let isCheckingUpdates = $state(false);
  let updateMessage = $state<{ type: "success" | "error"; text: string } | null>(null);

  async function loadSettings() {
    try {
      const settings = await invoke<{
        enable_undocumented_force_unlock: boolean;
        scan_level: string;
        backup_before_delete: boolean;
      }>("get_settings");
      
      scanLevel = settings.scan_level;
      autoBackup = settings.backup_before_delete;
      forceUnlock = settings.enable_undocumented_force_unlock;
      
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
          backup_before_delete: autoBackup
        }
      });
      toast.show("Cấu hình settings đã được lưu thành công!", "success");
    } catch (e: any) {
      toast.show(`Lỗi khi lưu settings: ${e.toString()}`, "error");
    }
  }

  async function checkUpdates() {
    isCheckingUpdates = true;
    updateMessage = null;
    setTimeout(() => {
      isCheckingUpdates = false;
      updateMessage = {
        type: "success",
        text: "You are running the latest version: PurgeKit v0.1.0-alpha!"
      };
    }, 1500);
  }

  async function reportIssue() {
    try {
      await openUrl("https://github.com/ThanhNguyxnOrg/PurgeKit/issues");
    } catch (e) {
      console.error(e);
      toast.show("Failed to open browser.", "error");
    }
  }

  onMount(() => {
    loadSettings();
  });
</script>

<div class="flex-1 flex flex-col h-screen bg-app-bg text-text-primary">
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
          <div class="flex flex-col">
            <span class="text-xs font-bold text-text-primary">Standard Mode Active</span>
            <span class="text-[10px] text-text-muted font-mono">Running as standard user. Some clean features will be restricted.</span>
          </div>
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
          <p class="text-xs text-text-muted font-mono">Version 0.1.0-alpha (Phase 1 Foundation)</p>
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
              {isCheckingUpdates ? 'Checking...' : 'Check for Updates'}
            </button>
            
            <button
              onclick={reportIssue}
              class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 transition-all text-text-primary cursor-pointer"
            >
              <Bug class="w-3.5 h-3.5 text-text-muted" />
              Report Issue
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

