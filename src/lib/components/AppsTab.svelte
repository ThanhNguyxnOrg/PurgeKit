<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Search, RefreshCw, Trash2, Laptop, ShoppingBag, Eye, X, CheckSquare, Square, Folder, Database, File } from "@lucide/svelte";

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
  }

  interface RemnantItem {
    path: string;
    item_type: string; // "File" | "Directory" | "RegistryKey"
    size: number;
  }

  let apps = $state<InstalledApp[]>([]);
  let searchQuery = $state("");
  let filterType = $state<"all" | "desktop" | "uwp">("all");
  let isLoading = $state(true);

  // Remnants Modal States
  let activeApp = $state<InstalledApp | null>(null);
  let remnants = $state<RemnantItem[]>([]);
  let selectedRemnants = $state<Record<string, boolean>>({});
  let isScanningRemnants = $state(false);
  let isPurging = $state(false);
  let purgeResult = $state<{ success: number; fail: number } | null>(null);

  // Fetch apps from Tauri backend
  async function loadApps() {
    isLoading = true;
    try {
      apps = await invoke<InstalledApp[]>("get_installed_apps");
    } catch (e) {
      console.error("Failed to load apps:", e);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadApps();
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

  // Handle standard uninstall
  async function handleUninstall(app: InstalledApp) {
    if (!app.uninstall_string) {
      alert("No uninstall string available for this application.");
      return;
    }
    
    try {
      // Execute the uninstaller
      await invoke("run_uninstall_command", { uninstall_string: app.uninstall_string });
    } catch (e: any) {
      alert(`Error starting uninstaller: ${e.toString()}`);
    }
  }

  // Open remnants scan modal
  async function handleOpenRemnantsScan(app: InstalledApp) {
    activeApp = app;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
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
    } finally {
      isScanningRemnants = false;
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
      alert("Please select at least one remnant item to purge.");
      return;
    }

    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge,
      });
      purgeResult = result;
      // Reload apps list
      await loadApps();
    } catch (e: any) {
      alert(`Error purging remnants: ${e.toString()}`);
    } finally {
      isPurging = false;
    }
  }

  function closeRemnantsModal() {
    activeApp = null;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
  }
</script>

<div class="flex-1 flex flex-col h-screen bg-app-bg text-text-primary relative">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        Apps Manager
      </h2>
      <p class="text-xs text-text-muted mt-1">Manage and clean standard desktop apps & Windows Store apps</p>
    </div>

    <button
      onclick={loadApps}
      disabled={isLoading}
      class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg hover:border-border-strong disabled:opacity-50 transition-all text-text-primary"
    >
      <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
      Reload List
    </button>
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
        <span class="text-sm text-text-muted font-mono">Scanning system registry & packages...</span>
      </div>
    {:else if filteredApps.length === 0}
      <div class="flex flex-col items-center justify-center h-64 border border-dashed border-border-default rounded-lg bg-surface-bg/30">
        <span class="text-sm text-text-muted">No applications matched your search criteria.</span>
      </div>
    {:else}
      <div class="border border-border-default rounded-lg overflow-hidden bg-surface-bg/50">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="bg-sidebar-bg border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
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
              <tr class="hover:bg-elevated-bg/50 transition-colors duration-150 group">
                <!-- Name -->
                <td class="py-4 px-5 font-sans font-medium flex items-center gap-3">
                  <div class="w-8 h-8 rounded-lg border flex items-center justify-center font-mono text-xs transition-colors shadow-sm {getAvatarStyle(app.display_name)}">
                    {app.display_name.charAt(0).toUpperCase()}
                  </div>
                  <div class="flex flex-col">
                    <span class="text-text-primary group-hover:text-white transition-colors">{app.display_name}</span>
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
                <td class="py-4 px-5 text-right">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      onclick={() => handleUninstall(app)}
                      title="Run official uninstaller"
                      class="p-2 rounded-lg bg-surface-bg hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all border border-border-default"
                    >
                      <Eye class="w-4 h-4" />
                    </button>
                    <button
                      onclick={() => handleOpenRemnantsScan(app)}
                      title="Deep clean remnants (Purge)"
                      class="p-2 rounded-lg bg-danger/10 hover:bg-danger/20 text-danger transition-all border border-transparent"
                    >
                      <Trash2 class="w-4 h-4" />
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
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
          {#if isScanningRemnants}
            <div class="flex flex-col items-center justify-center py-12 gap-3">
              <RefreshCw class="w-7 h-7 text-accent animate-spin" />
              <span class="text-xs text-text-muted font-mono">Analyzing directories & registry paths...</span>
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
                      <div class="flex items-center gap-2 mt-1">
                        <span class="text-[9px] font-mono uppercase px-1 rounded bg-app-bg border border-border-default text-text-muted">
                          {item.item_type}
                        </span>
                        {#if item.size > 0}
                          <span class="text-[9px] font-mono text-text-muted">{formatSize(item.size)}</span>
                        {/if}
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
</div>
