<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Search, RefreshCw, Trash2, Laptop, ShoppingBag, Eye, X, CheckSquare, Square, Folder, Database, File } from "@lucide/svelte";
  import { toast } from "../toast.svelte";

  interface InstalledApp {
    id: string;
    display_name: string;
    display_version: string | null;
    publisher: string | null;
    uninstall_string: string | null;
    quiet_uninstall_string: string | null;
    install_location: string | null;
    install_date: string | null;
    display_icon: string | null;
    estimated_size: number | null;
    registry_path: string;
    hive: string;
    is_uwp: boolean;
    icon_base64?: string | null;
    is_verified?: boolean | null;
  }

  interface RemnantItem {
    path: string;
    item_type: string; // "File" | "Directory" | "RegistryKey"
    size: number;
    confidence: string;
    score: number;
  }


  let apps = $state<InstalledApp[]>([]);
  let searchQuery = $state("");
  let filterType = $state<"all" | "desktop" | "uwp">("all");
  let isLoading = $state(true);
  let statusMessage = $state("Scanning system registry & packages...");

  // Remnants Modal States
  let activeApp = $state<InstalledApp | null>(null);
  let remnants = $state<RemnantItem[]>([]);
  let selectedRemnants = $state<Record<string, boolean>>({});
  let isScanningRemnants = $state(false);
  let isPurging = $state(false);
  let purgeResult = $state<{ success: number; fail: number } | null>(null);

  // App Checkbox Selection States
  let selectedApps = $state<Record<string, boolean>>({});
  let selectedAppsCount = $derived(Object.values(selectedApps).filter(Boolean).length);

  function toggleSelectApp(id: string) {
    selectedApps[id] = !selectedApps[id];
  }

  function toggleSelectAllApps(checked: boolean) {
    if (checked) {
      filteredApps.forEach(app => {
        selectedApps[app.id] = true;
      });
    } else {
      selectedApps = {};
    }
  }

  function clearAppSelection() {
    selectedApps = {};
  }

  // Bulk Uninstall States
  let isBulkUninstalling = $state(false);
  let bulkUninstallStep = $state<"idle" | "running" | "remnants_list" | "done">("idle");
  let bulkUninstallLog = $state<string[]>([]);
  let bulkAutoPurge = $state(true);
  let bulkRemnants = $state<RemnantItem[]>([]);
  let selectedBulkRemnants = $state<Record<string, boolean>>({});
  let bulkUninstallCurrent = $state(0);
  let bulkUninstallTotal = $state(0);
  let bulkUninstallCurrentAppName = $state("");
  let bulkUninstallStatusMessage = $state("");

  // Fetch apps from Tauri backend
  async function loadApps() {
    isLoading = true;
    try {
      apps = await invoke<InstalledApp[]>("get_installed_apps");
    } catch (e) {
      console.error("Failed to load apps:", e);
      toast.show("Error scanning application list!", "error");
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadApps();

    const unlistenScan = listen<{ phase: string; current: number; total: number; message: string }>(
      "scan-progress",
      (event) => {
        statusMessage = event.payload.message;
      }
    );

    const unlistenRemnant = listen<{ phase: string; current: number; total: number; message: string }>(
      "remnant-scan-progress",
      (event) => {
        if (uninstallStep === "scanning") {
          uninstallStatusText = event.payload.message;
        }
      }
    );

    const unlistenPurge = listen<{ phase: string; current: number; total: number; message: string }>(
      "purge-progress",
      (event) => {
        if (isPurging) {
          uninstallStatusText = event.payload.message;
        }
      }
    );

    const unlistenBulk = listen<{ app_id: string; app_name: string; phase: string; current: number; total: number; message: string }>(
      "bulk-uninstall-progress",
      (event) => {
        const payload = event.payload;
        bulkUninstallCurrent = payload.current;
        bulkUninstallTotal = payload.total;
        bulkUninstallCurrentAppName = payload.app_name;
        bulkUninstallStatusMessage = payload.message;
        
        const timestamp = new Date().toLocaleTimeString();
        bulkUninstallLog = [...bulkUninstallLog, `[${timestamp}] [${payload.phase.toUpperCase()}] ${payload.message}`];
      }
    );

    return () => {
      unlistenScan.then((fn) => fn());
      unlistenRemnant.then((fn) => fn());
      unlistenPurge.then((fn) => fn());
      unlistenBulk.then((fn) => fn());
    };
  });

  // Filter and search logic
  let filteredApps = $derived(
    apps.filter((app) => {
      const matchesSearch = app.display_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (app.publisher && app.publisher.toLowerCase().includes(searchQuery.toLowerCase()));
      
      const matchesFilter = filterType === "all" ||
        (filterType === "desktop" && !app.is_uwp) ||
        (filterType === "uwp" && app.is_uwp);

      return matchesSearch && matchesFilter;
    })
  );

  // Helper to format file size
  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes === 0) return "Unknown";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Helper to generate consistent avatar background and text color based on name
  function getAvatarStyle(name: string): string {
    const charCode = name.charCodeAt(0) || 0;
    const colors = [
      "bg-emerald-950/40 text-emerald-400 border-emerald-800/30",
      "bg-blue-950/40 text-blue-400 border-blue-800/30",
      "bg-brand-purple/10 text-brand-purple border-brand-purple/20",
      "bg-amber-950/40 text-amber-400 border-amber-800/30",
      "bg-rose-950/40 text-rose-400 border-rose-800/30",
    ];
    return colors[charCode % colors.length];
  }

  let uninstallStep = $state("idle");
  let uninstallStatusText = $state("");

  // Handle standard uninstall and leftovers scanning flow
  async function startUninstallFlow(app: InstalledApp) {
    activeApp = app;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    isScanningRemnants = false;
    
    // Step 1: Run official uninstaller (if present)
    if (app.uninstall_string) {
      uninstallStep = "uninstalling";
      uninstallStatusText = "Executing the official uninstaller. Please follow the uninstallation GUI prompts on your screen...";
      try {
        await invoke("run_uninstall_command", { uninstallString: app.uninstall_string });
      } catch (e: any) {
        console.error("Official uninstaller failed or was closed:", e);
        toast.show("The official uninstaller returned an error or was closed.", "warning");
      }
    }

    // Step 2: Scan for remnants
    uninstallStep = "scanning";
    uninstallStatusText = "Scanning system directories and registry paths for leftover traces...";
    isScanningRemnants = true;
    
    try {
      remnants = await invoke<RemnantItem[]>("get_app_remnants", {
        appName: app.display_name,
        publisher: app.publisher,
        installLocation: app.install_location,
      });

      // Select all by default
      remnants.forEach((item) => {
        selectedRemnants[item.path] = true;
      });
    } catch (e) {
      console.error("Failed to scan remnants:", e);
      toast.show("Error scanning remnant files on the system!", "error");
    } finally {
      isScanningRemnants = false;
      uninstallStep = "remnants_list";
    }
  }

  // Toggle selection for single item
  function toggleSelection(path: string) {
    selectedRemnants[path] = !selectedRemnants[path];
  }

  // Toggle select all
  function selectAll(val: boolean) {
    remnants.forEach((item) => {
      selectedRemnants[item.path] = val;
    });
  }

  // Perform purge remnants
  async function performPurge() {
    const itemsToPurge = remnants.filter((r) => selectedRemnants[r.path]);
    if (itemsToPurge.length === 0) {
      toast.show("Please select at least one remnant item to clean.", "warning");
      return;
    }

    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge,
      });
      purgeResult = result;
      toast.show(`Purge completed! Successfully removed ${result.success} remnant items.`, "success");
      // Reload apps list
      await loadApps();
    } catch (e: any) {
      toast.show(`Error purging remnants: ${e.toString()}`, "error");
    } finally {
      isPurging = false;
    }
  }


  function closeRemnantsModal() {
    activeApp = null;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    uninstallStep = "idle";
  }

  function exportApps(format: "csv" | "json") {
    if (filteredApps.length === 0) {
      toast.show("No application data to export!", "warning");
      return;
    }

    let content = "";
    let mimeType = "";
    let filename = "";

    if (format === "json") {
      content = JSON.stringify(filteredApps, null, 2);
      mimeType = "application/json";
      filename = `purgekit_apps_${new Date().toISOString().slice(0, 10)}.json`;
    } else {
      const headers = ["ID", "Name", "Version", "Publisher", "InstallLocation", "RegistryPath", "Hive", "IsStore", "IsVerified"];
      const rows = filteredApps.map((app) => [
        app.id,
        app.display_name,
        app.display_version || "",
        app.publisher || "",
        app.install_location || "",
        app.registry_path,
        app.hive,
        app.is_uwp ? "TRUE" : "FALSE",
        app.is_verified === true ? "TRUE" : app.is_verified === false ? "FALSE" : "UNKNOWN"
      ]);
      
      const csvContent = [
        headers.join(","),
        ...rows.map((row) => row.map((val) => `"${val.replace(/"/g, '""')}"`).join(","))
      ].join("\n");
      
      content = "\uFEFF" + csvContent;
      mimeType = "text/csv;charset=utf-8;";
      filename = `purgekit_apps_${new Date().toISOString().slice(0, 10)}.csv`;
    }

    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.setAttribute("href", url);
    link.setAttribute("download", filename);
    link.style.visibility = "hidden";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    
    toast.show(`Successfully exported report to ${format.toUpperCase()}!`, "success");
  }

  async function startBulkUninstall() {
    const selectedIds = Object.entries(selectedApps).filter(([_, val]) => val).map(([id, _]) => id);
    if (selectedIds.length === 0) return;
    
    isBulkUninstalling = true;
    bulkUninstallStep = "running";
    bulkUninstallLog = [`Initializing bulk silent uninstallation for ${selectedIds.length} application(s)...`];
    bulkRemnants = [];
    selectedBulkRemnants = {};
    
    try {
      const remainingRemnants = await invoke<RemnantItem[]>("run_bulk_silent_uninstall", {
        appIds: selectedIds,
        autoPurge: bulkAutoPurge,
      });
      
      bulkRemnants = remainingRemnants;
      
      bulkRemnants.forEach((item) => {
        selectedBulkRemnants[item.path] = true;
      });
      
      if (bulkRemnants.length > 0) {
        bulkUninstallStep = "remnants_list";
      } else {
        bulkUninstallStep = "done";
        toast.show("Bulk uninstallation and auto-purge completed! No remaining leftovers found.", "success");
        clearAppSelection();
        await loadApps();
      }
    } catch (e: any) {
      console.error("Bulk silent uninstallation failed:", e);
      toast.show(`Bulk uninstallation error: ${e.toString()}`, "error");
      bulkUninstallStep = "done";
    }
  }

  async function performBulkRemnantsPurge() {
    const itemsToPurge = bulkRemnants.filter((r) => selectedBulkRemnants[r.path]);
    if (itemsToPurge.length === 0) {
      toast.show("No leftovers selected to clean. Closing.", "info");
      bulkUninstallStep = "done";
      clearAppSelection();
      await loadApps();
      return;
    }
    
    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge,
      });
      toast.show(`Purge completed! Removed ${result.success} leftover items.`, "success");
      bulkUninstallStep = "done";
      clearAppSelection();
      await loadApps();
    } catch (e: any) {
      toast.show(`Error purging leftovers: ${e.toString()}`, "error");
    } finally {
      isPurging = false;
    }
  }

  function closeBulkModal() {
    isBulkUninstalling = false;
    bulkUninstallStep = "idle";
    bulkUninstallLog = [];
    bulkRemnants = [];
    selectedBulkRemnants = {};
    clearAppSelection();
    loadApps();
  }
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary relative">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        Apps Manager
      </h2>
      <p class="text-xs text-text-muted mt-1">Manage and clean standard desktop apps & Windows Store apps</p>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={loadApps}
        disabled={isLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg hover:border-border-strong disabled:opacity-50 transition-all text-text-primary cursor-pointer active:scale-95"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
        Reload List
      </button>

      <!-- Export Dropdown -->
      <div class="relative inline-block text-left group">
        <button
          class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-primary cursor-pointer active:scale-95"
        >
          <svg class="w-3.5 h-3.5 text-text-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Export
        </button>
        <div class="absolute right-0 mt-1.5 w-32 rounded-lg bg-surface-bg border border-border-default shadow-lg opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-150 z-50 p-1 flex flex-col gap-0.5">
          <button
            onclick={() => exportApps("csv")}
            class="w-full text-left px-2.5 py-1.5 rounded-md hover:bg-elevated-bg text-xs text-text-secondary hover:text-text-primary transition-colors font-sans font-semibold cursor-pointer"
          >
            Export to CSV
          </button>
          <button
            onclick={() => exportApps("json")}
            class="w-full text-left px-2.5 py-1.5 rounded-md hover:bg-elevated-bg text-xs text-text-secondary hover:text-text-primary transition-colors font-sans font-semibold cursor-pointer"
          >
            Export to JSON
          </button>
        </div>
      </div>
    </div>
  </header>

  <!-- Filter Toolbar -->
  <div class="p-6 pb-2 flex flex-col md:flex-row gap-4 justify-between items-stretch">
    <!-- Search Bar -->
    <div class="relative flex-1 max-w-md">
      <Search class="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
      <input
        type="text"
        placeholder="Search apps by name or publisher..."
        bind:value={searchQuery}
        class="w-full pl-10 pr-4 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-sm font-sans transition-all text-text-primary placeholder:text-text-muted"
      />
    </div>

    <!-- Filter Buttons -->
    <div class="flex bg-sidebar-bg p-1 rounded-lg border border-border-default self-start md:self-auto">
      <button
        onclick={() => filterType = "all"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold flex items-center gap-1.5 transition-all
          {filterType === 'all' ? 'bg-accent-subtle text-accent border border-accent/20' : 'text-text-secondary hover:text-text-primary'}"
      >
        All ({apps.length})
      </button>
      <button
        onclick={() => filterType = "desktop"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold flex items-center gap-1.5 transition-all
          {filterType === 'desktop' ? 'bg-accent-subtle text-accent border border-accent/20' : 'text-text-secondary hover:text-text-primary'}"
      >
        <Laptop class="w-3.5 h-3.5" />
        Desktop ({apps.filter(a => !a.is_uwp).length})
      </button>
      <button
        onclick={() => filterType = "uwp"}
        class="px-4 py-1.5 rounded-md text-xs font-semibold flex items-center gap-1.5 transition-all
          {filterType === 'uwp' ? 'bg-accent-subtle text-accent border border-accent/20' : 'text-text-secondary hover:text-text-primary'}"
      >
        <ShoppingBag class="w-3.5 h-3.5" />
        Windows Store ({apps.filter(a => a.is_uwp).length})
      </button>
    </div>
  </div>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto px-6 pb-6">
    {#if isLoading}
      <div class="flex flex-col items-center justify-center h-64 gap-3">
        <RefreshCw class="w-8 h-8 text-accent animate-spin" />
        <span class="text-sm text-text-muted font-mono">{statusMessage}</span>
      </div>

    {:else if filteredApps.length === 0}
      <div class="flex flex-col items-center justify-center h-64 border border-dashed border-border-default rounded-lg bg-surface-bg/30">
        <span class="text-sm text-text-muted">No applications matched your search criteria.</span>
      </div>
    {:else}
      <div class="border border-border-default rounded-lg overflow-hidden bg-surface-bg/50">
        <div class="overflow-x-auto w-full">
          <table class="w-full text-left border-collapse min-w-[800px]">
            <thead>
              <tr class="bg-sidebar-bg border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
                <th class="py-4 px-5 w-10">
                  <input
                    type="checkbox"
                    aria-label="Select all applications"
                    checked={selectedAppsCount === filteredApps.length && filteredApps.length > 0}
                    onchange={(e) => toggleSelectAllApps(e.currentTarget.checked)}
                    class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4"
                  />
                </th>
                <th class="py-4 px-5">App Name</th>
                <th class="py-4 px-5">Publisher</th>
                <th class="py-4 px-5">Version</th>
                <th class="py-4 px-5">Size</th>
                <th class="py-4 px-5">Registry Hive</th>
                <th class="py-4 px-5 text-right">Actions</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border-default text-sm">
              {#each filteredApps as app}
                <tr class="hover:bg-elevated-bg/50 transition-colors duration-150 group {selectedApps[app.id] ? 'bg-accent/5' : ''}">
                  <td class="py-4 px-5 w-10">
                    <input
                      type="checkbox"
                      aria-label="Select application {app.display_name}"
                      checked={!!selectedApps[app.id]}
                      onchange={() => toggleSelectApp(app.id)}
                      class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4"
                    />
                  </td>
                  <!-- Name -->
                  <td class="py-4 px-5 font-sans font-medium flex items-center gap-3">
                    <div class="w-8 h-8 rounded-lg border flex items-center justify-center overflow-hidden transition-colors shadow-sm bg-surface-bg border-border-default flex-shrink-0">
                      {#if app.icon_base64}
                        <img src="data:image/png;base64,{app.icon_base64}" class="w-6 h-6 object-contain" alt="" />
                      {:else}
                        <div class="w-full h-full flex items-center justify-center font-mono text-xs {getAvatarStyle(app.display_name)}">
                          {app.display_name.charAt(0).toUpperCase()}
                        </div>
                      {/if}
                    </div>
                    <div class="flex flex-col min-w-0">
                      <div class="flex items-center gap-1.5">
                        <span class="text-text-primary group-hover:text-white transition-colors max-w-[180px] md:max-w-[280px] truncate" title={app.display_name}>{app.display_name}</span>
                        {#if app.is_verified}
                          <span class="inline-flex items-center" title="Verified Publisher Certificate">
                            <svg class="w-3.5 h-3.5 text-emerald-400 fill-emerald-400/20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                              <path d="m9 11 2 2 4-4"/>
                            </svg>
                          </span>
                        {/if}
                      </div>
                      <span class="text-[10px] text-text-muted font-mono mt-0.5 max-w-[200px] truncate" title={app.registry_path}>
                        {app.id}
                      </span>
                    </div>
                  </td>
                  
                  <!-- Publisher -->
                  <td class="py-4 px-5 text-text-secondary max-w-[150px] truncate">
                    {app.publisher || "Unknown"}
                  </td>
                  
                  <!-- Version -->
                  <td class="py-4 px-5 text-text-secondary font-mono text-xs">
                    {app.display_version || "—"}
                  </td>
                  
                  <!-- Size -->
                  <td class="py-4 px-5 text-text-secondary font-mono text-xs">
                    {formatSize(app.estimated_size)}
                  </td>
                  
                  <!-- Hive / Type -->
                  <td class="py-4 px-5">
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono border
                      {app.is_uwp 
                        ? 'bg-blue-950/30 text-blue-400 border-blue-800/40' 
                        : app.hive === 'HKCU' 
                          ? 'bg-brand-purple/10 text-brand-purple border-brand-purple/20' 
                          : 'bg-surface-bg text-text-secondary border-border-default'}">
                      {app.is_uwp ? "STORE" : app.hive}
                    </span>
                  </td>
                  
                  <!-- Actions -->
                  <td class="py-4 px-5 text-right font-sans">
                    <div class="flex items-center justify-end gap-2">
                      <button
                        onclick={() => startUninstallFlow(app)}
                        title="Uninstall and Deep Clean Remnants"
                        class="px-2.5 py-1.5 rounded-lg bg-danger/10 hover:bg-danger text-danger hover:text-white transition-all border border-danger/20 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                      >
                        <Trash2 class="w-3.5 h-3.5" />
                        Uninstall
                      </button>
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>

  <!-- Remnants Modal Overlay -->
  {#if activeApp}
    <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4">
      <div class="bg-surface-bg border border-border-default w-full max-w-2xl rounded-xl flex flex-col max-h-[85vh] shadow-2xl relative overflow-hidden animate-scale-up duration-200">
        <!-- Header -->
        <header class="p-6 border-b border-border-default flex items-center justify-between">
          <div>
            <h3 class="text-base font-bold text-text-primary">
              Deep Clean remnants for: <span class="text-accent">{activeApp.display_name}</span>
            </h3>
            <p class="text-xs text-text-muted mt-1">Review files, directories, and registry keys to be permanently purged.</p>
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
                <h4 class="text-sm font-bold text-text-primary">Official Uninstaller Running</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed font-sans">{uninstallStatusText}</p>
              </div>
            </div>
          {:else if uninstallStep === "scanning"}
            <div class="flex flex-col items-center justify-center py-12 gap-4 text-center animate-fade-in">
              <RefreshCw class="w-8 h-8 text-accent animate-spin" />
              <div>
                <h4 class="text-sm font-bold text-text-primary">Scanning Leftover Traces</h4>
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
                  <h4 class="text-sm font-bold text-text-primary">Purge Completed!</h4>
                  <p class="text-xs text-text-muted mt-1">
                    Successfully removed <b class="text-success">{purgeResult.success}</b> remnants.
                    {#if purgeResult.fail > 0}
                      <span class="text-danger">Failed to remove {purgeResult.fail} items (locked or permission issue).</span>
                    {/if}
                  </p>
                </div>
              </div>
            {/if}

            {#if remnants.length === 0}
              {#if !purgeResult}
                <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center">
                  <span class="text-sm font-sans text-text-primary font-medium">No Remnants Detected!</span>
                  <span class="text-xs text-text-muted mt-1">This application was successfully uninstalled without leaving residual traces.</span>
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
                    Select All
                  </button>
                  <span class="text-border-default">|</span>
                  <button
                    onclick={() => selectAll(false)}
                    class="text-text-secondary hover:text-text-primary font-semibold"
                  >
                    Deselect All
                  </button>
                </div>
                <span class="text-text-muted font-mono">
                  {remnants.filter(r => selectedRemnants[r.path]).length} of {remnants.length} items selected
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
                          {item.confidence === 'VeryHigh' ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40' : ''}
                          {item.confidence === 'High' ? 'bg-blue-950/30 text-blue-400 border-blue-800/40' : ''}
                          {item.confidence === 'Medium' ? 'bg-amber-950/30 text-amber-400 border-amber-800/40' : ''}
                          {item.confidence === 'Low' ? 'bg-rose-950/30 text-rose-400 border-rose-800/40' : ''}"
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
              Total selected: <b class="text-accent">{formatSize(remnants.filter(r => selectedRemnants[r.path]).reduce((acc, curr) => acc + curr.size, 0))}</b>
            </span>
            <div class="flex gap-3">
              <button
                onclick={closeRemnantsModal}
                class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all"
              >
                Cancel
              </button>
              <button
                onclick={performPurge}
                disabled={isPurging || remnants.filter(r => selectedRemnants[r.path]).length === 0}
                class="flex items-center gap-2 px-5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow"
              >
                {#if isPurging}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                  Purging remnants...
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                  Purge Selected Remnants
                {/if}
              </button>
            </div>
          </footer>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Floating Bulk Action Bar -->
  {#if selectedAppsCount >= 2 && !isBulkUninstalling}
    <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 w-full max-w-2xl px-4 animate-slide-up">
      <div class="backdrop-blur-md bg-sidebar-bg/95 border border-border-strong rounded-xl p-4 shadow-2xl flex items-center justify-between gap-4">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 rounded-lg bg-accent/10 border border-accent/20 flex items-center justify-center text-accent">
            <CheckSquare class="w-4.5 h-4.5" />
          </div>
          <div>
            <h4 class="text-xs font-bold text-text-primary">{selectedAppsCount} apps selected</h4>
            <p class="text-[10px] text-text-muted mt-0.5 font-sans">Ready for batch silent uninstallation</p>
          </div>
        </div>

        <div class="flex items-center gap-4">
          <!-- Auto-Purge Toggle -->
          <label class="flex items-center gap-2 cursor-pointer select-none">
            <input
              type="checkbox"
              bind:checked={bulkAutoPurge}
              class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4"
            />
            <div class="flex flex-col">
              <span class="text-xs font-semibold text-text-secondary">Auto-Purge Leftovers</span>
              <span class="text-[9px] text-text-muted font-sans font-medium">Delete remnants silently</span>
            </div>
          </label>

          <div class="h-8 w-[1px] bg-border-default"></div>

          <div class="flex items-center gap-2">
            <button
              onclick={clearAppSelection}
              class="px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all active:scale-95 cursor-pointer"
            >
              Cancel
            </button>
            <button
              onclick={startBulkUninstall}
              class="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-bold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 shadow cursor-pointer"
            >
              <Trash2 class="w-3.5 h-3.5" />
              Bulk Silent Uninstall
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Bulk Uninstall Modal -->
  {#if isBulkUninstalling}
    <div class="absolute inset-0 bg-app-bg/60 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in">
      <div class="bg-sidebar-bg border border-border-strong w-full max-w-3xl rounded-xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh] animate-scale-up">
        <!-- Header -->
        <header class="p-6 border-b border-border-default flex items-center justify-between">
          <div>
            <h3 class="text-base font-bold text-text-primary">
              Bulk Silent Uninstallation
            </h3>
            <p class="text-xs text-text-muted mt-1">
              {#if bulkUninstallStep === "running"}
                Processing <b>{bulkUninstallCurrent}/{bulkUninstallTotal}</b>: <span class="text-accent">{bulkUninstallCurrentAppName}</span>
              {:else if bulkUninstallStep === "remnants_list"}
                Review remaining leftovers from batch uninstallation
              {:else}
                Batch operations completed
              {/if}
            </p>
          </div>
          {#if bulkUninstallStep === "done"}
            <button
              onclick={closeBulkModal}
              class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
            >
              <X class="w-4 h-4" />
            </button>
          {/if}
        </header>

        <!-- Progress Bar -->
        {#if bulkUninstallStep === "running" && bulkUninstallTotal > 0}
          <div class="h-1 bg-border-default w-full relative overflow-hidden">
            <div
              class="h-full bg-accent transition-all duration-300"
              style="width: {(bulkUninstallCurrent / bulkUninstallTotal) * 100}%"
            ></div>
          </div>
        {/if}

        <!-- Body -->
        <div class="flex-1 overflow-y-auto p-6 space-y-4">
          {#if bulkUninstallStep === "running"}
            <div class="flex flex-col gap-3">
              <div class="flex items-center gap-3 p-3.5 rounded-lg bg-accent/5 border border-accent/10">
                <RefreshCw class="w-5 h-5 text-accent animate-spin flex-shrink-0" />
                <div class="flex-1 min-w-0">
                  <div class="flex items-center justify-between text-xs font-semibold text-text-primary">
                    <span>{bulkUninstallCurrentAppName}</span>
                    <span class="font-mono text-accent">{bulkUninstallCurrent}/{bulkUninstallTotal}</span>
                  </div>
                  <p class="text-[11px] text-text-muted mt-1 truncate">{bulkUninstallStatusMessage}</p>
                </div>
              </div>

              <!-- Console-style Log -->
              <h4 class="text-xs font-bold text-text-secondary uppercase tracking-wider font-mono">Uninstall Log</h4>
              <div class="bg-app-bg border border-border-default rounded-lg p-4 font-mono text-[11px] text-text-muted overflow-y-auto h-[250px] space-y-1.5 select-text">
                {#each bulkUninstallLog as line}
                  <div class="leading-relaxed border-b border-border-default/20 pb-1 last:border-b-0 break-all">{line}</div>
                {/each}
              </div>
            </div>

          {:else if bulkUninstallStep === "remnants_list"}
            <!-- Residual Clean list -->
            <div class="space-y-3">
              <div class="p-4 rounded-lg bg-amber-950/20 border border-amber-900/40 text-amber-300 text-xs flex gap-3 leading-relaxed">
                <span class="text-base">⚠️</span>
                <div>
                  <h4 class="font-bold">Remaining Leftovers Detected</h4>
                  <p class="mt-1 text-text-muted">The uninstaller completed, but some confidence-level remnants couldn't be auto-purged (or auto-purge was disabled). Review and select below to clean them permanently.</p>
                </div>
              </div>

              <!-- Selection Toolbar -->
              <div class="flex items-center justify-between pb-1 text-xs border-b border-border-default">
                <div class="flex gap-2">
                  <button
                    onclick={() => {
                      bulkRemnants.forEach(r => selectedBulkRemnants[r.path] = true);
                    }}
                    class="text-accent hover:text-accent-hover font-semibold cursor-pointer"
                  >
                    Select All
                  </button>
                  <span class="text-border-default">|</span>
                  <button
                    onclick={() => {
                      selectedBulkRemnants = {};
                    }}
                    class="text-text-secondary hover:text-text-primary font-semibold cursor-pointer"
                  >
                    Deselect All
                  </button>
                </div>
                <span class="text-text-muted font-mono">
                  {Object.values(selectedBulkRemnants).filter(Boolean).length} of {bulkRemnants.length} items selected
                </span>
              </div>

              <!-- Remnants List -->
              <div class="space-y-2 max-h-[280px] overflow-y-auto">
                {#each bulkRemnants as item}
                  <button
                    onclick={() => selectedBulkRemnants[item.path] = !selectedBulkRemnants[item.path]}
                    class="w-full flex items-start gap-3 p-3 rounded-lg border text-left transition-all hover:bg-elevated-bg/50 cursor-pointer
                      {selectedBulkRemnants[item.path] ? 'bg-accent-subtle/30 border-accent/20' : 'bg-surface-bg border-border-default'}"
                  >
                    <div class="mt-0.5 text-accent flex-shrink-0">
                      {#if selectedBulkRemnants[item.path]}
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
                          {item.confidence === 'VeryHigh' ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40' : ''}
                          {item.confidence === 'High' ? 'bg-blue-950/30 text-blue-400 border-blue-800/40' : ''}
                          {item.confidence === 'Medium' ? 'bg-amber-950/30 text-amber-400 border-amber-800/40' : ''}
                          {item.confidence === 'Low' ? 'bg-rose-950/30 text-rose-400 border-rose-800/40' : ''}"
                        >
                          {item.confidence} ({item.score}%)
                        </span>
                      </div>
                    </div>
                  </button>
                {/each}
              </div>
            </div>

          {:else if bulkUninstallStep === "done"}
            <div class="py-12 flex flex-col items-center justify-center text-center gap-4 animate-fade-in">
              <div class="w-16 h-16 rounded-full bg-success/10 border border-success/20 flex items-center justify-center text-success shadow-inner">
                <svg class="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
              </div>
              <div>
                <h4 class="text-base font-bold text-text-primary">All Applications Processed!</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed">
                  Bulk uninstallation and leftovers cleaning completed successfully.
                </p>
              </div>
            </div>
          {/if}
        </div>

        <!-- Footer -->
        {#if bulkUninstallStep === "remnants_list" || bulkUninstallStep === "done"}
          <footer class="p-6 border-t border-border-default flex justify-end items-center gap-3 bg-sidebar-bg">
            {#if bulkUninstallStep === "remnants_list"}
              <button
                onclick={() => bulkUninstallStep = "done"}
                class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all cursor-pointer"
              >
                Skip Cleaning
              </button>
              <button
                onclick={performBulkRemnantsPurge}
                disabled={isPurging}
                class="flex items-center gap-1.5 px-5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 disabled:opacity-50 cursor-pointer"
              >
                {#if isPurging}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                  Purging leftovers...
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                  Clean Leftover Traces
                {/if}
              </button>
            {:else}
              <button
                onclick={closeBulkModal}
                class="px-6 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 cursor-pointer"
              >
                Done
              </button>
            {/if}
          </footer>
        {/if}
      </div>
    </div>
  {/if}
</div>
