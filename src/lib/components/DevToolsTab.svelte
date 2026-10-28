<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { Terminal, RefreshCw, Trash2, HardDrive, AlertCircle, CheckCircle, FolderOpen, X, Square, CheckSquare, Folder, Database, File, Cpu } from "@lucide/svelte";
  import { toast } from "../toast.svelte";
  import { t } from "$lib/i18n.svelte";

  interface DevToolInfo {
    name: string;
    detected: boolean;
    version: string | null;
    path: string | null;
    cache_path: string | null;
    cache_size: number | null;
    clean_command: string | null;
  }

  interface PathEntry {
    value: string;
    is_valid: boolean;
    scope: string;
  }

  interface GlobalCliPackage {
    name: string;
    version: string;
    size_bytes: number;
    path: string;
    description: string | null;
    manager: string; // "npm" | "yarn" | "pnpm" | "bun"
  }

  interface RemnantItem {
    path: string;
    item_type: string; // "File", "Directory", "RegistryKey"
    size: number;
    confidence: string; // "VeryHigh" | "High" | "Medium" | "Low"
    score: number;
  }

  interface ToolchainVersion {
    manager: string;
    version: string;
    path: string;
    size_bytes: number;
    is_active: boolean;
  }

  interface DevExtension {
    ide: string;
    name: string;
    version: string | null;
    path: string;
    size_bytes: number;
  }

  let activeSubTab = $state("caches"); // "caches" | "packages" | "toolchains" | "extensions"

  let tools = $state<DevToolInfo[]>([]);
  let pathIssuesCount = $state(0);
  let isLoading = $state(true);
  let cleaningStates = $state<Record<string, boolean>>({});

  let globalPackages = $state<GlobalCliPackage[]>([]);
  let isPackagesLoading = $state(false);
  let uninstallingStates = $state<Record<string, boolean>>({});

  // Toolchains States
  let toolchains = $state<ToolchainVersion[]>([]);
  let isToolchainsLoading = $state(false);
  let uninstallingToolchains = $state<Record<string, boolean>>({});

  // IDE extension inventory
  let extensions = $state<DevExtension[]>([]);
  let isExtensionsLoading = $state(false);

  // Remnants Modal States
  let activePackage = $state<GlobalCliPackage | null>(null);
  let remnants = $state<RemnantItem[]>([]);
  let selectedRemnants = $state<Record<string, boolean>>({});
  let isScanningRemnants = $state(false);
  let isPurging = $state(false);
  let uninstallStep = $state<"uninstalling" | "scanning" | "remnants_list" | "completed" | null>(null);
  let uninstallStatusText = $state("");
  let purgeResult = $state<{ success: number; fail: number } | null>(null);

  async function loadDevTools() {
    isLoading = true;
    try {
      tools = await invoke<DevToolInfo[]>("get_dev_tools");
      
      // Load path issues count for the stats row
      const envPaths = await invoke<PathEntry[]>("get_path_entries");
      pathIssuesCount = envPaths.filter(p => !p.is_valid).length;
    } catch (e) {
      console.error("Failed to load dev tools data:", e);
    } finally {
      isLoading = false;
    }
  }

  async function loadGlobalPackages() {
    isPackagesLoading = true;
    try {
      globalPackages = await invoke<GlobalCliPackage[]>("get_global_cli_packages");
      globalPackages.sort((a, b) => a.name.localeCompare(b.name));
    } catch (e) {
      console.error("Failed to load global packages:", e);
      toast.show(t("Failed to load global CLI packages!"), "error");
    } finally {
      isPackagesLoading = false;
    }
  }

  async function loadToolchains() {
    isToolchainsLoading = true;
    try {
      toolchains = await invoke<ToolchainVersion[]>("get_toolchain_versions");
      toolchains.sort((a, b) => b.size_bytes - a.size_bytes);
    } catch (e) {
      console.error("Failed to load toolchains:", e);
      toast.show(t("Failed to load compiler toolchains!"), "error");
    } finally {
      isToolchainsLoading = false;
    }
  }

  async function loadExtensions() {
    isExtensionsLoading = true;
    try {
      extensions = await invoke<DevExtension[]>("get_dev_extensions");
    } catch (e) {
      console.error("Failed to load IDE extensions:", e);
      toast.show(t("Failed to load IDE extensions!"), "error");
    } finally {
      isExtensionsLoading = false;
    }
  }

  function refreshAll() {
    loadDevTools();
    loadGlobalPackages();
    loadToolchains();
    loadExtensions();
  }

  async function uninstallPackage(pkg: GlobalCliPackage) {
    if (!confirm(t("Uninstall global package \"{name}\" from {manager}?", { name: pkg.name, manager: pkg.manager }))) {
      return;
    }

    activePackage = pkg;
    uninstallStep = "uninstalling";
    uninstallStatusText = t("Uninstalling global package: {name}...", { name: pkg.name });
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    isPurging = false;
    
    try {
      // 1. Get binary names before we uninstall it and delete package.json
      uninstallStatusText = t("Analyzing binary files for {name}...", { name: pkg.name });
      const binNames = await invoke<string[]>("get_cli_package_bin_names", { packagePath: pkg.path });
      
      // 2. Perform official uninstallation
      uninstallStatusText = t("Running official uninstallation command for {name}...", { name: pkg.name });
      await invoke("uninstall_global_cli_package", { name: pkg.name, manager: pkg.manager });
      
      // 3. Scan for leftovers
      uninstallStep = "scanning";
      uninstallStatusText = t("Searching system paths for remnants of {name}...", { name: pkg.name });
      
      // Allow a small delay for the OS to release file handles if any
      await new Promise(resolve => setTimeout(resolve, 500));
      
      remnants = await invoke<RemnantItem[]>("get_cli_package_remnants", {
        name: pkg.name,
        manager: pkg.manager,
        packagePath: pkg.path,
        binNames
      });
      
      // Only pre-select the strongest evidence. Bin shims/config folders are
      // left unchecked because they may be shared or user-owned.
      remnants.forEach(item => {
        selectedRemnants[item.path] = item.confidence === "VeryHigh";
      });
      
      uninstallStep = "remnants_list";
    } catch (err: any) {
      console.error(err);
      toast.show(t("Failed during uninstallation: {error}", { error: err.toString() }), "error");
      closeRemnantsModal();
    }
  }

  function selectAll(val: boolean) {
    remnants.forEach(item => {
      selectedRemnants[item.path] = val;
    });
  }

  function toggleSelection(path: string) {
    selectedRemnants[path] = !selectedRemnants[path];
  }

  async function performPurge() {
    const itemsToPurge = remnants.filter(r => selectedRemnants[r.path]);
    if (itemsToPurge.length === 0) {
      toast.show(t("Please select at least one remnant item to clean."), "warning");
      return;
    }

    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge
      });
      purgeResult = result;
      toast.show(t("Purged {count} items successfully!", { count: result.success }), "success");
      
      // Refresh global packages list
      await loadGlobalPackages();
    } catch (e: any) {
      console.error(e);
      toast.show(t("Failed to purge remnants: {error}", { error: e.toString() }), "error");
    } finally {
      isPurging = false;
    }
  }

  function closeRemnantsModal() {
    activePackage = null;
    remnants = [];
    selectedRemnants = {};
    uninstallStep = null;
    purgeResult = null;
  }

  onMount(() => {
    loadDevTools();
    loadGlobalPackages();
    loadToolchains();
    loadExtensions();
  });

  // Helper to format file size
  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes === undefined) return t("N/A");
    if (bytes <= 0 || isNaN(bytes)) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Handle clean single dev cache
  async function cleanCache(name: string) {
    cleaningStates[name] = true;
    try {
      const freedBytes = await invoke<number>("clean_dev_tool_cache", { name });
      toast.show(t("Successfully cleaned cache for {name}. Freed {size}!", { name, size: formatSize(freedBytes) }), "success");
      // Reload dev tools info to reflect changes
      await loadDevTools();
    } catch (err: any) {
      toast.show(t("Failed to clean {name} cache: {error}", { name, error: err.toString() }), "error");
    } finally {
      cleaningStates[name] = false;
    }
  }

  // Clean all detected caches
  async function cleanAllCaches() {
    const detectedTools = tools.filter(x => x.detected && x.cache_size && x.cache_size > 0);
    if (detectedTools.length === 0) {
      toast.show(t("All caches are already clean!"), "success");
      return;
    }

    const totalSize = detectedTools.reduce((sum, tool) => sum + (tool.cache_size || 0), 0);
    if (!confirm(t("Clean caches for {count} tools ({size})?", { count: detectedTools.length, size: formatSize(totalSize) }))) {
      return;
    }

    let totalFreed = 0;
    let failedTools: string[] = [];

    for (const tool of detectedTools) {
      cleaningStates[tool.name] = true;
      try {
        const freed = await invoke<number>("clean_dev_tool_cache", { name: tool.name });
        totalFreed += freed;
      } catch (err) {
        console.error(`Error cleaning ${tool.name}:`, err);
        failedTools.push(tool.name);
      } finally {
        cleaningStates[tool.name] = false;
      }
    }

    await loadDevTools();

    if (failedTools.length > 0) {
      toast.show(t("Freed {size} but failed on: {tools}", { size: formatSize(totalFreed), tools: failedTools.join(", ") }), "error");
    } else {
      toast.show(t("Successfully cleaned all caches! Total freed: {size}", { size: formatSize(totalFreed) }), "success");
    }
  }

  let totalCacheSize = $derived(
    tools.filter(x => x.detected).reduce((acc, curr) => acc + (curr.cache_size || 0), 0)
  );

  let detectedToolsCount = $derived(
    tools.filter(x => x.detected).length
  );

  let totalToolchainsSize = $derived(
    toolchains.reduce((acc, curr) => acc + curr.size_bytes, 0)
  );

  let totalToolchainsCount = $derived(
    toolchains.length
  );

  let totalExtensionsSize = $derived(
    extensions.reduce((sum, extension) => sum + extension.size_bytes, 0)
  );

  async function uninstallToolchain(tc: ToolchainVersion) {
    let confirmMsg = t("Are you sure you want to uninstall {manager} version: {version}?", { manager: tc.manager, version: tc.version });
    if (tc.is_active) {
      confirmMsg = `${t("WARNING: The toolchain {version} is currently ACTIVE in your environment.", { version: tc.version })}\n\n${t("Uninstalling it might break your active developer projects until you configure another version.")}\n\n${t("Are you sure you want to proceed?")}`;
    }
    if (!confirm(confirmMsg)) {
      return;
    }

    uninstallingToolchains[tc.path] = true;
    try {
      toast.show(t("Uninstalling {version}...", { version: tc.version }), "info");
      await invoke("delete_toolchain_version", {
        manager: tc.manager,
        version: tc.version,
        path: tc.path
      });
      toast.show(t("Successfully uninstalled {version}!", { version: tc.version }), "success");
      await loadToolchains();
    } catch (err: any) {
      toast.show(t("Failed to uninstall toolchain: {error}", { error: err.toString() }), "error");
    } finally {
      uninstallingToolchains[tc.path] = false;
    }
  }
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        {t("Dev Tools")}
      </h2>
      <p class="text-xs text-text-muted mt-1">{t("Free up storage by cleaning unused package caches, compiler versions & global packages")}</p>
    </div>

    <div class="flex items-center gap-2">
      {#if activeSubTab === 'caches'}
        <button
          onclick={cleanAllCaches}
          disabled={isLoading || tools.filter(x => x.detected && x.cache_size && x.cache_size > 0).length === 0}
          class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-accent-strong hover:bg-accent-strong/90 text-white disabled:opacity-50 active:scale-95 transition-all shadow"
        >
          <Trash2 class="w-3.5 h-3.5" />
          {t("Purge All Caches")}
        </button>
      {/if}

      <button
        onclick={refreshAll}
        disabled={isLoading || isPackagesLoading || isToolchainsLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 disabled:opacity-50 transition-all text-text-primary"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading || isPackagesLoading || isToolchainsLoading ? 'animate-spin' : ''}" />
        {t("Refresh")}
      </button>
    </div>
  </header>

  <!-- Sub-Tab Navigation Selector -->
  <div class="px-6 py-3 border-b border-border-default bg-sidebar-bg/60 flex items-center gap-4">
    <div class="flex p-1 rounded-lg bg-app-bg border border-border-default w-max">
      <button
        onclick={() => activeSubTab = "caches"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold transition-all duration-150 cursor-pointer
          {activeSubTab === 'caches' ? 'bg-surface-bg border border-border-default text-accent shadow-sm' : 'text-text-secondary hover:text-text-primary border border-transparent'}"
      >
        {t("Developer Caches")}
      </button>
      <button
        onclick={() => activeSubTab = "packages"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold transition-all duration-150 cursor-pointer
          {activeSubTab === 'packages' ? 'bg-surface-bg border border-border-default text-accent shadow-sm' : 'text-text-secondary hover:text-text-primary border border-transparent'}"
      >
        {t("Global CLI Packages")}
      </button>
      <button
        onclick={() => activeSubTab = "toolchains"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold transition-all duration-150 cursor-pointer
          {activeSubTab === 'toolchains' ? 'bg-surface-bg border border-border-default text-accent shadow-sm' : 'text-text-secondary hover:text-text-primary border border-transparent'}"
      >
        {t("Toolchain Runtimes")}
      </button>
      <button
        onclick={() => activeSubTab = "extensions"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold transition-all duration-150 cursor-pointer
          {activeSubTab === 'extensions' ? 'bg-surface-bg border border-border-default text-accent shadow-sm' : 'text-text-secondary hover:text-text-primary border border-transparent'}"
      >
        {t("IDE Extensions")}
      </button>
    </div>
  </div>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">

    {#if activeSubTab === 'caches'}
      <!-- Tab 1: Caches Grid -->
      {#if isLoading}
        <div class="flex flex-col items-center justify-center h-64 gap-3">
          <RefreshCw class="w-8 h-8 text-accent animate-spin" />
          <span class="text-sm text-text-muted font-mono">{t("Analyzing local developer caches...")}</span>
        </div>
      {:else}
        <!-- Stats Row -->
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <!-- Total Cache Card -->
          <div class="p-4 rounded-lg bg-surface-bg border-l-4 border-l-accent border border-border-default flex items-center gap-3 shadow-sm">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <HardDrive class="w-5 h-5 text-accent" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">{t("Total Cache")}</span>
              <span class="text-lg font-bold text-text-primary">{formatSize(totalCacheSize)}</span>
            </div>
          </div>

          <!-- Tools Found Card -->
          <div class="p-4 rounded-lg bg-surface-bg border-l-4 border-l-border-strong border border-border-default flex items-center gap-3 shadow-sm">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <Terminal class="w-5 h-5 text-text-secondary" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">{t("Tools Found")}</span>
              <span class="text-lg font-bold text-text-primary">{t("{count} Detected", { count: detectedToolsCount })}</span>
            </div>
          </div>

          <!-- PATH Issues Card -->
          <div class="p-4 rounded-lg bg-surface-bg border-l-4 border-l-danger border border-border-default flex items-center gap-3 shadow-sm">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center animate-fade-in">
              <AlertCircle class="w-5 h-5 text-danger" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">{t("PATH Issues")}</span>
              <span class="text-lg font-bold text-danger">{t("{count} Broken", { count: pathIssuesCount })}</span>
            </div>
          </div>
        </div>

        <!-- Tools Grid -->
        {#if tools.filter(x => x.detected).length === 0}
          <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center py-16">
            <Terminal class="w-8 h-8 text-text-muted mb-3 animate-pulse" />
            <span class="text-sm font-sans text-text-primary font-medium">{t("No Developer Caches Detected")}</span>
            <span class="text-xs text-text-muted mt-1 max-w-sm">{t("We couldn't find any supported developer toolchains (like npm, cargo, docker, conda, etc.) in your system's PATH.")}</span>
          </div>
        {:else}
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {#each tools.filter(x => x.detected) as tool}
              <div class="border border-border-default hover:border-border-strong rounded-lg p-5 flex flex-col bg-surface-bg/50 transition-all duration-200 relative overflow-hidden group">
                
                <!-- Card Header -->
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center shadow-sm">
                      <Terminal class="w-5 h-5 text-accent" />
                    </div>
                    <div>
                      <h3 class="text-base font-bold font-sans capitalize text-text-primary">{tool.name}</h3>
                      <span class="text-[10px] font-mono text-success">
                        {t("Version")} {tool.version}
                      </span>
                    </div>
                  </div>
                  
                  <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-success/10 text-success border border-success/20">
                    {t("Ready")}
                  </span>
                </div>

                <!-- Path details -->
                <div class="mt-4 flex-1 space-y-2.5 text-xs">
                  <div class="space-y-1">
                    <span class="text-text-muted font-mono text-[9px] uppercase tracking-wide">{t("Cache Path")}</span>
                    <div class="flex items-center gap-1.5 text-text-secondary bg-app-bg p-2 rounded-lg border border-border-default font-mono text-[10px] select-all truncate" title={tool.cache_path}>
                      <FolderOpen class="w-3.5 h-3.5 text-text-muted flex-shrink-0" />
                      <span class="truncate">{tool.cache_path}</span>
                    </div>
                  </div>

                  <div class="flex items-center justify-between bg-app-bg/50 p-3 rounded-lg border border-border-default">
                    <div class="flex items-center gap-2">
                      <HardDrive class="w-4 h-4 text-text-secondary" />
                      <span class="text-text-secondary font-sans">{t("Current Cache Size")}</span>
                    </div>
                    <span class="text-sm font-bold font-mono text-accent">
                      {formatSize(tool.cache_size)}
                    </span>
                  </div>
                </div>

                <!-- Actions -->
                <div class="mt-5 pt-4 border-t border-border-default flex gap-2">
                  <button
                    onclick={() => cleanCache(tool.name)}
                    disabled={cleaningStates[tool.name] || !tool.cache_size || tool.cache_size === 0}
                    class="w-full flex items-center justify-center gap-2 py-2 rounded-lg text-xs font-semibold bg-surface-bg hover:bg-elevated-bg border border-border-default text-text-primary disabled:opacity-40 transition-all active:scale-[0.98]"
                  >
                    {#if cleaningStates[tool.name]}
                      <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                      {t("Cleaning...")}
                    {:else}
                      <Trash2 class="w-3.5 h-3.5" />
                      {t("Purge Cache")}
                    {/if}
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

    {:else if activeSubTab === 'packages'}
      <!-- Tab 2: Global CLI Packages -->
      <div class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
        <div class="flex items-center justify-between border-b border-border-default pb-3">
          <div>
            <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
              <Terminal class="w-4 h-4 text-accent" />
              {t("Globally Installed CLI Packages")}
            </h3>
            <span class="text-[10px] text-text-muted">{t("Detected global packages across NPM, Yarn, PNPM, Bun registries")}</span>
          </div>
          
          <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-accent/10 text-accent border border-accent/20">
            {t("{count} Packages", { count: globalPackages.length })}
          </span>
        </div>

        {#if isPackagesLoading}
          <div class="flex items-center justify-center py-6 gap-2">
            <RefreshCw class="w-4 h-4 text-accent animate-spin" />
            <span class="text-xs text-text-muted font-mono">{t("Scanning global packages...")}</span>
          </div>
        {:else if globalPackages.length === 0}
          <p class="text-xs text-text-muted italic py-4 text-center font-sans">
            {t("No globally installed CLI packages detected in NPM, Yarn, PNPM, or Bun directories.")}
          </p>
        {:else}
          <div class="divide-y divide-border-default">
            {#each globalPackages as pkg}
              <div class="flex items-center justify-between py-3.5 first:pt-0 last:pb-0 gap-4 group">
                <div class="flex items-center gap-3 flex-1 min-w-0">
                  <div class="w-8 h-8 rounded-lg border flex items-center justify-center overflow-hidden transition-colors shadow-sm bg-surface-bg border-border-default flex-shrink-0 relative">
                    <!-- Try loading the NPM package logo from unavatar.io, fallback to initials otherwise -->
                    <img src="https://unavatar.io/npm/{pkg.name}" 
                         onload={(e) => (e.currentTarget as HTMLImageElement).style.opacity = '1'}
                         class="w-6 h-6 object-contain opacity-0 transition-opacity duration-150 relative z-10" 
                         alt="" />
                    <div class="absolute inset-0 flex items-center justify-center font-mono text-[10px] bg-danger/10 text-danger border border-danger/25 w-full h-full">
                      {pkg.name.charAt(0).toUpperCase()}
                    </div>
                  </div>

                  <div class="flex-1 min-w-0 font-sans">
                     <div class="flex items-center gap-2">
                       <span class="font-bold text-xs font-mono text-text-primary truncate" title={pkg.name}>{pkg.name}</span>
                       <span class="text-[10px] font-mono text-text-muted bg-app-bg px-1.5 py-0.5 rounded border border-border-default">v{pkg.version}</span>
                       <span class="text-[9px] uppercase px-1.5 py-0.5 rounded font-mono font-semibold
                         {pkg.manager === 'npm' ? 'bg-danger/10 text-danger border border-danger/20' : ''}
                         {pkg.manager === 'yarn' ? 'bg-info/10 text-info border border-info/20' : ''}
                         {pkg.manager === 'pnpm' ? 'bg-warning/10 text-warning border border-warning/20' : ''}
                         {pkg.manager === 'bun' ? 'bg-pink-500/10 text-pink-400 border border-pink-500/20' : ''}
                         {pkg.manager === 'pip' ? 'bg-success/10 text-success border border-success/20' : ''}
                         {pkg.manager === 'cargo' ? 'bg-orange-500/10 text-orange-400 border border-orange-500/20' : ''}
                         {pkg.manager === 'go' ? 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/20' : ''}
                         {pkg.manager === 'dotnet' ? 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/20' : ''}
                         {pkg.manager === 'composer' ? 'bg-purple-500/10 text-purple-400 border border-purple-500/20' : ''}
                         {pkg.manager === 'deno' ? 'bg-surface-bg text-text-secondary border border-border-default' : ''}"
                       >
                         {pkg.manager}
                       </span>
                     </div>
                     {#if pkg.description}
                       <p class="text-[11px] text-text-muted mt-1 truncate" title={pkg.description}>{pkg.description}</p>
                     {/if}
                     <div class="text-[9px] text-text-disabled font-mono mt-0.5 truncate" title={pkg.path}>{pkg.path}</div>
                  </div>
                </div>

                <div class="flex items-center gap-4">
                  <span class="text-xs font-bold font-mono text-text-secondary whitespace-nowrap">{formatSize(pkg.size_bytes)}</span>
                  <button
                    onclick={() => uninstallPackage(pkg)}
                    class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[10px] font-semibold bg-danger/10 hover:bg-danger-strong text-danger hover:text-white transition-all duration-150 active:scale-95 border border-danger/20"
                  >
                    <Trash2 class="w-3 h-3" />
                    {t("Uninstall")}
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    {:else if activeSubTab === 'toolchains'}
      <!-- Tab 3: Toolchain Runtimes -->
      <div class="space-y-6 animate-fade-in">
        <!-- Stats Row -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <HardDrive class="w-5 h-5 text-accent" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">{t("Total Runtime Space")}</span>
              <span class="text-lg font-bold text-text-primary">{formatSize(totalToolchainsSize)}</span>
            </div>
          </div>

          <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <Cpu class="w-5 h-5 text-text-secondary" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">{t("Runtimes Detected")}</span>
              <span class="text-lg font-bold text-text-primary">{t("{count} Versions", { count: totalToolchainsCount })}</span>
            </div>
          </div>
        </div>

        <!-- Toolchains List -->
        <div class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4">
          <div class="flex items-center justify-between border-b border-border-default pb-3">
            <div>
              <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
                <Cpu class="w-4 h-4 text-accent" />
                {t("Compiler & Runtime Versions")}
              </h3>
              <span class="text-[10px] text-text-muted">{t("Clean up old versions of Rustup, NVM, and FNM managers to free up storage")}</span>
            </div>
            <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-accent/10 text-accent border border-accent/20 font-mono">
              {t("{count} Versions", { count: toolchains.length })}
            </span>
          </div>

          {#if isToolchainsLoading}
            <div class="flex items-center justify-center py-6 gap-2">
              <RefreshCw class="w-4 h-4 text-accent animate-spin" />
              <span class="text-xs text-text-muted font-mono">{t("Scanning toolchain runtimes...")}</span>
            </div>
          {:else if toolchains.length === 0}
            <p class="text-xs text-text-muted italic py-4 text-center font-sans">
              {t("No installed compiler toolchains detected (Rustup, NVM, or FNM).")}
            </p>
          {:else}
            <div class="divide-y divide-border-default">
              {#each toolchains as tc}
                <div class="flex items-center justify-between py-3.5 first:pt-0 last:pb-0 gap-4 group animate-fade-in">
                  <div class="flex items-center gap-3 flex-1 min-w-0">
                    <div class="w-8 h-8 rounded-lg border flex items-center justify-center bg-surface-bg border-border-default flex-shrink-0">
                      <Cpu class="w-4.5 h-4.5 text-accent" />
                    </div>

                    <div class="flex-1 min-w-0 font-sans">
                      <div class="flex items-center gap-2">
                        <span class="font-bold text-xs font-mono text-text-primary truncate" title={tc.version}>{tc.version}</span>
                        <span class="text-[9px] uppercase px-1.5 py-0.5 rounded font-mono font-semibold
                          {tc.manager === 'rustup' ? 'bg-orange-500/10 text-orange-400 border border-orange-500/20' : ''}
                          {tc.manager === 'nvm' ? 'bg-info/10 text-info border border-info/20' : ''}
                          {tc.manager === 'fnm' ? 'bg-pink-500/10 text-pink-400 border border-pink-500/20' : ''}"
                        >
                          {tc.manager}
                        </span>
                        
                        {#if tc.is_active}
                          <span class="text-[9px] bg-success/15 border border-success/35 text-success px-1.5 py-0.5 rounded font-mono font-semibold">
                            {t("Active")}
                          </span>
                        {/if}
                      </div>
                      <div class="text-[9px] text-text-disabled font-mono mt-0.5 truncate" title={tc.path}>{tc.path}</div>
                    </div>
                  </div>

                  <div class="flex items-center gap-4">
                    <span class="text-xs font-bold font-mono text-text-secondary whitespace-nowrap">{formatSize(tc.size_bytes)}</span>
                    <button
                      onclick={() => uninstallToolchain(tc)}
                      disabled={uninstallingToolchains[tc.path]}
                      class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[10px] font-semibold bg-danger/10 hover:bg-danger-strong text-danger hover:text-white transition-all duration-150 active:scale-95 border border-danger/20 disabled:opacity-50 cursor-pointer"
                    >
                      {#if uninstallingToolchains[tc.path]}
                        <RefreshCw class="w-3 h-3 animate-spin" />
                        {t("Uninstalling...")}
                      {:else}
                        <Trash2 class="w-3 h-3" />
                        {t("Uninstall")}
                      {/if}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

    {:else if activeSubTab === 'extensions'}
      <div class="border border-border-default rounded-lg bg-surface-bg/30 p-5 space-y-4 animate-fade-in">
        <div class="flex items-center justify-between gap-4 border-b border-border-default pb-3">
          <div>
            <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
              <Terminal class="w-4 h-4 text-accent" />
              {t("IDE Extensions")}
            </h3>
            <span class="text-[11px] text-text-muted">
              {t("VS Code, Cursor, Windsurf and JetBrains plugin inventories")}
            </span>
          </div>
          <div class="flex items-center gap-3">
            <span class="text-xs font-mono text-text-secondary">
              {t("{count} extensions · {size}", { count: extensions.length, size: formatSize(totalExtensionsSize) })}
            </span>
            <button
              onclick={loadExtensions}
              disabled={isExtensionsLoading}
              class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg transition-colors"
              aria-label={t("Refresh IDE extensions")}
            >
              <RefreshCw class="w-3.5 h-3.5 text-text-secondary {isExtensionsLoading ? 'animate-spin text-accent' : ''}" />
            </button>
          </div>
        </div>

        {#if isExtensionsLoading}
          <div class="flex items-center justify-center py-8 gap-2">
            <RefreshCw class="w-4 h-4 text-accent animate-spin" />
            <span class="text-xs text-text-muted font-mono">{t("Scanning IDE extension folders...")}</span>
          </div>
        {:else if extensions.length === 0}
          <div class="py-8 text-center text-xs text-text-muted border border-dashed border-border-default rounded-lg">
            {t("No IDE extensions detected in the common VS Code/JetBrains locations.")}
          </div>
        {:else}
          <div class="divide-y divide-border-default max-h-[520px] overflow-y-auto pr-1">
            {#each extensions as extension}
              <div class="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0">
                <div class="flex-1 min-w-0">
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="text-xs font-semibold text-text-primary truncate" title={extension.name}>{extension.name}</span>
                    {#if extension.version}
                      <span class="text-[10px] font-mono text-text-muted">v{extension.version}</span>
                    {/if}
                    <span class="text-[9px] font-mono uppercase px-1.5 py-0.5 rounded border border-border-default text-text-muted">{extension.ide}</span>
                  </div>
                  <div class="text-[10px] font-mono text-text-disabled mt-1 truncate" title={extension.path}>{extension.path}</div>
                </div>
                <span class="text-xs font-mono text-text-secondary whitespace-nowrap">{formatSize(extension.size_bytes)}</span>
                <button
                  onclick={() => openPath(extension.path)}
                  class="px-2.5 py-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary text-[10px] font-semibold transition-colors"
                  aria-label={t("Open {name} folder", { name: extension.name })}
                >
                  {t("Open")}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

  </div>
</div>

<!-- Remnants Modal Overlay -->
{#if activePackage}
  <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4 animate-fade-in">
    <div class="bg-surface-bg border border-border-default w-full max-w-2xl rounded-xl flex flex-col max-h-[85vh] shadow-2xl relative overflow-hidden animate-scale-up duration-200">
      <!-- Header -->
      <header class="p-6 border-b border-border-default flex items-center justify-between">
        <div>
          <h3 class="text-base font-bold text-text-primary">
            {t("Deep Clean remnants for:")} <span class="text-accent">{activePackage.name}</span>
          </h3>
          <p class="text-xs text-text-muted mt-1">{t("Review files, directories, and configuration settings to be permanently purged.")}</p>
        </div>
        <button
          onclick={closeRemnantsModal}
          class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </header>

      <!-- Body -->
      <div class="flex-1 overflow-y-auto p-6 space-y-4">
        {#if uninstallStep === "uninstalling"}
          <div class="flex flex-col items-center justify-center py-12 gap-4 text-center animate-fade-in">
            <div class="w-12 h-12 rounded-full border-2 border-accent/20 border-t-accent animate-spin flex items-center justify-center">
              <Trash2 class="w-5 h-5 text-accent animate-pulse" />
            </div>
            <div>
              <h4 class="text-sm font-bold text-text-primary">{t("Official Uninstaller Running")}</h4>
              <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed font-sans">{uninstallStatusText}</p>
            </div>
          </div>
        {:else if uninstallStep === "scanning"}
          <div class="flex flex-col items-center justify-center py-12 gap-4 text-center animate-fade-in">
            <RefreshCw class="w-8 h-8 text-accent animate-spin" />
            <div>
              <h4 class="text-sm font-bold text-text-primary">{t("Scanning Leftover Traces")}</h4>
              <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed font-sans">{uninstallStatusText}</p>
            </div>
          </div>
        {:else}
          {#if purgeResult}
            <div class="p-6 rounded-lg bg-success/10 border border-success/20 flex flex-col items-center justify-center text-center gap-3">
              <div class="w-10 h-10 rounded-full bg-success/20 border border-success/30 flex items-center justify-center text-success animate-fade-in">
                <Trash2 class="w-5 h-5" />
              </div>
              <div>
                <h4 class="text-sm font-bold text-text-primary">{t("Purge Completed!")}</h4>
                <p class="text-xs text-text-muted mt-1">
                  {t("Successfully removed")} <b class="text-success">{purgeResult.success}</b> {t("remnants.")}
                  {#if purgeResult.fail > 0}
                    <span class="text-danger">{t("Failed to remove {count} items (locked or permission issue).", { count: purgeResult.fail })}</span>
                  {/if}
                </p>
              </div>
            </div>
          {/if}

          {#if remnants.length === 0}
            {#if !purgeResult}
              <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center">
                <span class="text-sm font-sans text-text-primary font-medium">{t("No Remnants Detected!")}</span>
                <span class="text-xs text-text-muted mt-1">{t("This package was successfully uninstalled without leaving residual traces.")}</span>
              </div>
            {/if}
          {:else}
            <!-- Toolbar selection -->
            <div class="flex items-center justify-between pb-2 text-xs border-b border-border-default">
              <div class="flex gap-2">
                <button
                  onclick={() => selectAll(true)}
                  class="text-accent hover:text-accent-hover font-semibold"
                >
                  {t("Select All")}
                </button>
                <span class="text-border-default">|</span>
                <button
                  onclick={() => selectAll(false)}
                  class="text-text-secondary hover:text-text-primary font-semibold"
                >
                  {t("Deselect All")}
                </button>
              </div>
              <span class="text-text-muted font-mono">
                {t("{selected} of {total} items selected", { selected: remnants.filter(r => selectedRemnants[r.path]).length, total: remnants.length })}
              </span>
            </div>

            <!-- List -->
            <div class="space-y-2">
              {#each remnants as item}
                <button
                  onclick={() => toggleSelection(item.path)}
                  class="w-full flex items-start gap-3 p-3 rounded-lg border text-left transition-all hover:bg-elevated-bg/50
                    {selectedRemnants[item.path] ? 'bg-accent-subtle/30 border-accent/20' : 'bg-surface-bg border-border-default'}"
                >
                  <div class="mt-0.5 text-accent flex-shrink-0">
                    {#if selectedRemnants[item.path]}
                      <CheckSquare class="w-4 h-4 text-accent" />
                    {:else}
                      <Square class="w-4 h-4 text-text-muted" />
                    {/if}
                  </div>

                  <div class="w-8 h-8 rounded bg-app-bg border border-border-default flex items-center justify-center text-text-secondary flex-shrink-0">
                    {#if item.item_type === 'Directory'}
                      <Folder class="w-4.5 h-4.5 text-warning" />
                    {:else if item.item_type === 'RegistryKey'}
                      <Database class="w-4.5 h-4.5 text-brand-purple" />
                    {:else}
                      <File class="w-4.5 h-4.5 text-info" />
                    {/if}
                  </div>

                  <div class="flex-1 min-w-0">
                    <span class="block text-xs font-mono font-medium text-text-primary break-all">{item.path}</span>
                    <div class="flex flex-wrap items-center gap-2 mt-1">
                      <span class="text-[9px] font-mono uppercase px-1 rounded bg-app-bg border border-border-default text-text-muted">
                        {item.item_type}
                      </span>
                      {#if item.size > 0}
                        <span class="text-[9px] font-mono text-text-muted">{formatSize(item.size)}</span>
                      {/if}
                      <span class="text-[9px] font-mono px-1 rounded border font-semibold
                        {item.confidence === 'VeryHigh' ? 'bg-success/10 text-success border-success/25' : ''}
                        {item.confidence === 'High' ? 'bg-info/10 text-info border-info/25' : ''}
                        {item.confidence === 'Medium' ? 'bg-warning/10 text-warning border-warning/25' : ''}
                        {item.confidence === 'Low' ? 'bg-danger/10 text-danger border-danger/25' : ''}"
                      >
                        {item.confidence} ({item.score}%)
                      </span>
                    </div>
                  </div>
                </button>
              {/each}
            </div>
          {/if}
        {/if}
      </div>

      <!-- Footer -->
      {#if !purgeResult && remnants.length > 0}
        <footer class="p-6 border-t border-border-default flex justify-between items-center bg-sidebar-bg">
          <span class="text-xs font-mono text-text-secondary">
            {t("Total selected:")} <b class="text-accent">{formatSize(remnants.filter(r => selectedRemnants[r.path]).reduce((acc, curr) => acc + curr.size, 0))}</b>
          </span>
          <div class="flex gap-3">
            <button
              onclick={closeRemnantsModal}
              class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all"
            >
              {t("Cancel")}
            </button>
            <button
              onclick={performPurge}
              disabled={isPurging || remnants.filter(r => selectedRemnants[r.path]).length === 0}
              class="flex items-center gap-2 px-5 py-2 rounded-lg text-xs font-semibold bg-accent-strong hover:bg-accent-strong/90 text-white disabled:opacity-50 active:scale-95 transition-all shadow"
            >
              {#if isPurging}
                <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                {t("Purging remnants...")}
              {:else}
                <Trash2 class="w-3.5 h-3.5" />
                {t("Purge Selected Remnants")}
              {/if}
            </button>
          </div>
        </footer>
      {:else if purgeResult || remnants.length === 0}
        <footer class="p-6 border-t border-border-default flex justify-end bg-sidebar-bg">
          <button
            onclick={closeRemnantsModal}
            class="px-5 py-2 rounded-lg text-xs font-semibold bg-accent-strong hover:bg-accent-strong/90 text-white active:scale-95 transition-all shadow"
          >
            {t("Close")}
          </button>
        </footer>
      {/if}
    </div>
  </div>
{/if}
