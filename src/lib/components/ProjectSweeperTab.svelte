<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { 
    FolderSearch, FolderPlus, Trash2, Play, X, Activity, 
    FileCode, RefreshCw, AlertTriangle, Check, Plus, Folder, 
    Terminal, Settings, ShieldAlert, CheckSquare, Square, HardDrive, Info
  } from "@lucide/svelte";
  import { toast } from "../toast.svelte";
  import { t } from "$lib/i18n.svelte";

  interface ProjectFolder {
    path: string;
    name: string;
    project_name: string;
    folder_type: string;
    size_bytes: number;
    file_count: number;
    last_modified: string;
  }

  interface ScanProgressEvent {
    current_dir: string;
    folders_found: number;
    total_size_bytes: number;
  }

  interface DeleteProgressEvent {
    phase: string;
    current: number;
    total: number;
    message: string;
  }

  // State bindings
  let roots = $state<string[]>([]);
  let newPath = $state("");
  let scanning = $state(false);
  let currentScanDir = $state("");
  let foldersFound = $state<ProjectFolder[]>([]);
  let selectedPaths = $state<Record<string, boolean>>({});
  
  let foldersFoundCount = $state(0);
  let totalBytesFound = $state(0);

  let deleting = $state(false);
  let deleteProgress = $state({ current: 0, total: 0, message: "" });
  let deleteReport = $state<Record<string, string>>({});
  let showReportModal = $state(false);

  // Folder types to sweep
  let folderTypes = $state([
    { id: "node_modules", label: "NodeJS Dependencies (node_modules)", checked: true, badge: "NodeJS", color: "text-danger bg-danger/10 border-danger/20" },
    { id: "target", label: "Rust Build Artifacts (target)", checked: true, badge: "Rust", color: "text-orange-400 bg-orange-950/20 border-orange-800/20" },
    { id: "venv", label: "Python Virtual Env (venv)", checked: true, badge: "Python", color: "text-info bg-info/10 border-info/20" },
    { id: ".venv", label: "Python Virtual Env (.venv)", checked: true, badge: "Python", color: "text-info bg-info/10 border-info/20" },
    { id: ".vs", label: "VS Cache & Output (.vs)", checked: true, badge: "C++", color: "text-purple-400 bg-purple-950/20 border-purple-800/20" },
    { id: "bin", label: "C# / .NET Binaries (bin)", checked: false, badge: "C#", color: "text-success bg-success/10 border-success/20" },
    { id: "obj", label: "C# / .NET Objects (obj)", checked: false, badge: "C#", color: "text-success bg-success/10 border-success/20" },
    { id: "build", label: "Java/Gradle Build Output (build)", checked: false, badge: "Gradle", color: "text-cyan-400 bg-cyan-950/20 border-cyan-800/20" },
    { id: ".gradle", label: "Gradle Cache (.gradle)", checked: false, badge: "Gradle", color: "text-cyan-400 bg-cyan-950/20 border-cyan-800/20" },
    { id: ".svelte-kit", label: "SvelteKit Cache (.svelte-kit)", checked: false, badge: "Svelte", color: "text-pink-400 bg-pink-950/20 border-pink-800/20" },
    { id: ".next", label: "Next.js Build Files (.next)", checked: false, badge: "Next.js", color: "text-indigo-400 bg-indigo-950/20 border-indigo-800/20" }
  ]);

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenDelete: UnlistenFn | null = null;

  async function loadSettings() {
    try {
      const settings = await invoke<any>("get_settings");
      roots = settings.project_scan_paths || [];
    } catch (e) {
      console.error("Failed to load settings scan paths:", e);
    }
  }

  async function saveSettingsPaths(updatedRoots: string[]) {
    try {
      const settings = await invoke<any>("get_settings");
      settings.project_scan_paths = updatedRoots;
      await invoke("save_settings", { settings });
    } catch (e: any) {
      toast.show(t("Error saving paths: {error}", { error: e.toString() }), "error");
    }
  }

  async function addRootPath() {
    const trimmed = newPath.trim();
    if (!trimmed) {
      toast.show(t("Please enter a valid directory path."), "warning");
      return;
    }

    const upperPath = trimmed.toUpperCase().replace(/\//g, "\\");
    const normalizedUpper = upperPath.replace(/\\+$/, "");
    
    // Block drive roots (like C:\ or D:)
    const driveRootRegex = /^[A-Z]:\\?$/;
    if (driveRootRegex.test(normalizedUpper)) {
      toast.show(t("Scanning drive roots directly is blocked for safety. Please select a specific folder inside the drive."), "error");
      return;
    }

    // Block base system folders and roots
    const blockedExact = [
      "C:\\WINDOWS",
      "C:\\PROGRAM FILES",
      "C:\\PROGRAM FILES (x86)",
      "C:\\PROGRAMDATA",
      "C:\\USERS"
    ];
    if (blockedExact.some(dir => normalizedUpper === dir)) {
      toast.show(t("Protected system directory cannot be swept directly for security reasons."), "error");
      return;
    }

    // Block scanning inside Windows, Program Files, and ProgramData
    const blockedPrefixes = [
      "C:\\WINDOWS\\",
      "C:\\PROGRAM FILES\\",
      "C:\\PROGRAM FILES (x86)\\",
      "C:\\PROGRAMDATA\\"
    ];
    if (blockedPrefixes.some(prefix => normalizedUpper.startsWith(prefix))) {
      toast.show(t("Protected system or program directory cannot be swept directly for security reasons."), "error");
      return;
    }

    try {
      // Validate path existence on the backend
      const exists = await invoke<boolean>("check_directory_exists", { path: trimmed });
      if (!exists) {
        toast.show(t("Directory path does not exist on your computer."), "error");
        return;
      }

      if (roots.includes(trimmed)) {
        toast.show(t("This path is already configured."), "warning");
        return;
      }

      const updated = [...roots, trimmed];
      roots = updated;
      newPath = "";
      await saveSettingsPaths(updated);
      toast.show(t("Scan root path added successfully!"), "success");
    } catch (e: any) {
      toast.show(t("Failed to validate path: {error}", { error: e.toString() }), "error");
    }
  }

  async function removeRootPath(pathToRemove: string) {
    const updated = roots.filter(r => r !== pathToRemove);
    roots = updated;
    await saveSettingsPaths(updated);
    toast.show(t("Scan root path removed."), "success");
  }

  async function startScan() {
    if (roots.length === 0) {
      toast.show(t("Please add at least one scan root directory first."), "warning");
      return;
    }

    const activeTypes = folderTypes.filter(type => type.checked).map(type => type.id);
    if (activeTypes.length === 0) {
      toast.show(t("Please check at least one target folder type to sweep."), "warning");
      return;
    }

    scanning = true;
    currentScanDir = t("Initializing...");
    foldersFound = [];
    selectedPaths = {};
    foldersFoundCount = 0;
    totalBytesFound = 0;

    try {
      // Listen to progress updates
      unlistenProgress = await listen<ScanProgressEvent>("project-scan-progress", (event) => {
        currentScanDir = event.payload.current_dir;
        foldersFoundCount = event.payload.folders_found;
        totalBytesFound = event.payload.total_size_bytes;
      });

      const results = await invoke<ProjectFolder[]>("scan_project_directories", {
        roots,
        folderTypes: activeTypes
      });

      // Sort by size descending
      results.sort((a, b) => b.size_bytes - a.size_bytes);
      foldersFound = results;

      // Pre-select all found folders for convenient purge
      foldersFound.forEach(f => {
        selectedPaths[f.path] = true;
      });

      toast.show(t("Scan complete! Found {count} project folders.", { count: foldersFound.length }), "success");
    } catch (e: any) {
      console.error(e);
      toast.show(t("Scan failed: {error}", { error: e.toString() }), "error");
    } finally {
      scanning = false;
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
    }
  }

  async function purgeSelected() {
    const selected = foldersFound.filter(f => selectedPaths[f.path]).map(f => f.path);
    if (selected.length === 0) {
      toast.show(t("No items selected for cleaning."), "warning");
      return;
    }

    const message = `${t("Are you sure you want to permanently delete the {count} selected build/dependency directories?", { count: selected.length })}\n${t("This will clear compiled binaries and packages, which can be re-installed/re-built later.")}`;
    if (!confirm(message)) {
      return;
    }

    deleting = true;
    deleteProgress = { current: 0, total: selected.length, message: t("Starting delete process...") };
    deleteReport = {};
    showReportModal = false;

    try {
      unlistenDelete = await listen<DeleteProgressEvent>("project-delete-progress", (event) => {
        deleteProgress = {
          current: event.payload.current,
          total: event.payload.total,
          message: event.payload.message
        };
      });

      const report = await invoke<Record<string, string>>("delete_project_directories", {
        paths: selected
      });

      deleteReport = report;
      showReportModal = true;

      // Filter out deleted items from the local results list
      foldersFound = foldersFound.filter(f => report[f.path] !== "Deleted" && report[f.path] !== "DeletedAfterUnlock" && report[f.path] !== "ForceDeleted");
      
      // Update selected records
      selectedPaths = {};
      foldersFound.forEach(f => {
        selectedPaths[f.path] = true;
      });

      toast.show(t("Project directories cleanup finished!"), "success");
    } catch (e: any) {
      console.error(e);
      toast.show(t("Purge failed: {error}", { error: e.toString() }), "error");
    } finally {
      deleting = false;
      if (unlistenDelete) {
        unlistenDelete();
        unlistenDelete = null;
      }
    }
  }

  // Selection helpers
  function selectAll(val: boolean) {
    foldersFound.forEach(f => {
      selectedPaths[f.path] = val;
    });
  }

  function toggleSelection(path: string) {
    selectedPaths[path] = !selectedPaths[path];
  }

  // Size formatting helper
  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes === undefined || bytes <= 0 || isNaN(bytes)) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Get badge type color class
  function getBadgeMeta(typeId: string) {
    const found = folderTypes.find(f => f.id === typeId);
    return found ? { badge: found.badge, color: found.color } : { badge: t("Unknown"), color: "text-text-muted bg-surface-bg/30" };
  }

  // Derived metrics
  let totalSelectedSize = $derived(
    foldersFound
      .filter(f => selectedPaths[f.path])
      .reduce((acc, curr) => acc + curr.size_bytes, 0)
  );

  let selectedCount = $derived(
    foldersFound.filter(f => selectedPaths[f.path]).length
  );

  let totalReclaimableBytes = $derived(
    foldersFound.reduce((acc, curr) => acc + curr.size_bytes, 0)
  );

  onMount(() => {
    loadSettings();
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenDelete) unlistenDelete();
  });
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <!-- Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        {t("Universal Project Sweeper")}
      </h2>
      <p class="text-xs text-text-muted mt-1">
        {t("Locate and purge heavy build artifacts, temporary caches, and dependency folders in programming directories.")}
      </p>
    </div>

    <div class="flex items-center gap-2">
      {#if scanning}
        <div class="flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold bg-accent-subtle/50 text-accent border border-accent/20">
          <RefreshCw class="w-3.5 h-3.5 animate-spin" />
          {t("Scanning...")}
        </div>
      {:else}
        <button
          onclick={startScan}
          disabled={roots.length === 0}
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold bg-accent-strong hover:bg-accent-strong/90 text-white disabled:opacity-50 active:scale-95 transition-all shadow cursor-pointer"
        >
          <Play class="w-3.5 h-3.5 fill-current" />
          {t("Start Sweep Scan")}
        </button>
      {/if}
    </div>
  </header>

  <!-- Main View Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    
    <!-- Config Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
      
      <!-- Roots Config Panel (5 cols) -->
      <div class="lg:col-span-5 border border-border-default rounded-xl bg-surface-bg/30 p-5 flex flex-col space-y-4">
        <div>
          <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider flex items-center gap-1.5">
            <Settings class="w-4 h-4 text-accent" />
            {t("Configured Scan Roots")}
          </h3>
          <p class="text-[10px] text-text-muted mt-1">{t("Add folders that contain your active programming projects.")}</p>
        </div>

        <!-- Path List -->
        <div class="space-y-2 max-h-[140px] overflow-y-auto pr-1 flex-1 min-h-[80px]">
          {#if roots.length === 0}
            <div class="py-6 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/20 text-xs text-text-muted">
              {t("No scan paths configured. Add one below.")}
            </div>
          {:else}
            {#each roots as root}
              <div class="flex items-center justify-between p-2 rounded-lg bg-app-bg border border-border-default text-xs font-mono group">
                <span class="truncate pr-3 text-text-secondary select-all" title={root}>{root}</span>
                <button
                  onclick={() => removeRootPath(root)}
                  class="p-1 rounded hover:bg-danger-strong/20 text-text-muted hover:text-danger transition-colors cursor-pointer"
                  title={t("Remove Path")}
                >
                  <X class="w-3.5 h-3.5" />
                </button>
              </div>
            {/each}
          {/if}
        </div>

        <!-- Add Input -->
        <div class="flex gap-2">
          <input
            type="text"
            bind:value={newPath}
            placeholder={t("e.g. D:\\Projects or D:\\Code")}
            class="flex-1 px-3 py-2 rounded-lg bg-app-bg border border-border-default text-xs text-text-primary outline-none focus:border-accent/50 font-mono"
            onkeydown={(e) => e.key === "Enter" && addRootPath()}
          />
          <button
            onclick={addRootPath}
            class="p-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg transition-all active:scale-95 cursor-pointer text-text-primary"
            title={t("Add directory path")}
          >
            <Plus class="w-4 h-4" />
          </button>
        </div>
      </div>

      <!-- Target Types Filter Panel (7 cols) -->
      <div class="lg:col-span-7 border border-border-default rounded-xl bg-surface-bg/30 p-5 flex flex-col space-y-4">
        <div>
          <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider flex items-center gap-1.5">
            <HardDrive class="w-4 h-4 text-accent" />
            {t("Target Directory Filters")}
          </h3>
          <p class="text-[10px] text-text-muted mt-1">{t("Select directory types to search for and purge.")}</p>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-2">
          {#each folderTypes as type}
            <button
              onclick={() => type.checked = !type.checked}
              class="flex items-center gap-2 p-2.5 rounded-lg border text-left transition-all hover:bg-elevated-bg/50 text-xs
                {type.checked ? 'bg-accent-subtle/20 border-accent/20 text-text-primary' : 'bg-surface-bg border-border-default text-text-secondary opacity-60'}"
            >
              <div class="text-accent flex-shrink-0">
                {#if type.checked}
                  <CheckSquare class="w-4 h-4 text-accent" />
                {:else}
                  <Square class="w-4 h-4 text-text-muted" />
                {/if}
              </div>
              <span class="truncate" title={t(type.label)}>{t(type.label)}</span>
            </button>
          {/each}
        </div>
      </div>

    </div>

    <!-- Active Scanning Progress Panel -->
    {#if scanning}
      <div class="border border-accent/20 rounded-xl bg-accent-subtle/10 p-5 space-y-3.5 animate-pulse">
        <div class="flex items-center justify-between text-xs">
          <span class="font-mono text-accent font-semibold flex items-center gap-2">
            <Activity class="w-4 h-4 animate-pulse" />
            {t("Active Scan Traversal")}
          </span>
          <span class="font-mono text-text-secondary">
            {t("Found:")} <b class="text-accent">{foldersFoundCount}</b> {t("directories")} ({formatSize(totalBytesFound)})
          </span>
        </div>
        <div class="h-1.5 w-full bg-app-bg rounded-full overflow-hidden border border-border-default">
          <div class="h-full bg-accent animate-infinite-loading w-1/3 rounded-full"></div>
        </div>
        <div class="text-[10px] font-mono text-text-muted truncate bg-app-bg p-2.5 rounded-lg border border-border-default">
          {t("Searching: {path}", { path: currentScanDir })}
        </div>
      </div>
    {/if}

    <!-- Scan Results -->
    {#if !scanning}
      {#if foldersFound.length === 0}
        <div class="py-16 text-center border border-dashed border-border-default rounded-xl bg-surface-bg/10 flex flex-col items-center justify-center space-y-3">
          <div class="w-12 h-12 rounded-lg bg-surface-bg border border-border-default flex items-center justify-center text-text-muted">
            <FolderSearch class="w-6 h-6" />
          </div>
          <div>
            <h4 class="text-sm font-bold text-text-primary">{t("No Project Directories Loaded")}</h4>
            <p class="text-xs text-text-muted mt-1">{t("Configure root paths and target filters, then run a Sweep Scan above.")}</p>
          </div>
        </div>
      {:else}
        <!-- Stats summary row -->
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          
          <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <HardDrive class="w-5 h-5 text-accent" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase">{t("Reclaimable Space")}</span>
              <span class="text-lg font-bold text-text-primary">{formatSize(totalReclaimableBytes)}</span>
            </div>
          </div>

          <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
              <Folder class="w-5 h-5 text-text-secondary" />
            </div>
            <div>
              <span class="block text-[10px] text-text-muted font-mono uppercase">{t("Total Folders Found")}</span>
              <span class="text-lg font-bold text-text-primary">{t("{count} Directories", { count: foldersFound.length })}</span>
            </div>
          </div>

          <div class="p-4 rounded-lg bg-accent-subtle/10 border border-accent/20 flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-accent/15 border border-accent/30 flex items-center justify-center text-accent">
              <CheckSquare class="w-5 h-5" />
            </div>
            <div>
              <span class="block text-[10px] text-accent/80 font-mono uppercase">{t("Selected for Purging")}</span>
              <span class="text-lg font-bold text-accent">{t("{count} items ({size})", { count: selectedCount, size: formatSize(totalSelectedSize) })}</span>
            </div>
          </div>

        </div>

        <!-- Table -->
        <div class="border border-border-default rounded-xl overflow-hidden bg-surface-bg/50">
          <!-- Selection Toolbar -->
          <div class="flex items-center justify-between p-4 bg-sidebar-bg border-b border-border-default text-xs font-mono">
            <div class="flex gap-3">
              <button
                onclick={() => selectAll(true)}
                class="text-accent hover:text-accent-hover font-semibold cursor-pointer"
              >
                {t("Select All")}
              </button>
              <span class="text-border-default">|</span>
              <button
                onclick={() => selectAll(false)}
                class="text-text-secondary hover:text-text-primary font-semibold cursor-pointer"
              >
                {t("Deselect All")}
              </button>
            </div>
            <span class="text-text-muted">
              {t("{selected} of {total} selected", { selected: selectedCount, total: foldersFound.length })}
            </span>
          </div>

          <div class="overflow-x-auto w-full">
            <table class="w-full text-left border-collapse min-w-[800px]">
              <thead>
                <tr class="bg-sidebar-bg/60 border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
                  <th class="py-3.5 px-5 w-10">{t("Select")}</th>
                  <th class="py-3.5 px-5">{t("Project Name")}</th>
                  <th class="py-3.5 px-5">{t("Clean Target")}</th>
                  <th class="py-3.5 px-5">{t("Size / Count")}</th>
                  <th class="py-3.5 px-5">{t("Last Modified")}</th>
                  <th class="py-3.5 px-5">{t("Directory Path")}</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border-default text-sm">
                {#each foldersFound as f}
                  {@const meta = getBadgeMeta(f.name)}
                  <tr class="hover:bg-elevated-bg/40 transition-colors duration-150 group">
                    <!-- Checkbox -->
                    <td class="py-3.5 px-5">
                      <button
                        onclick={() => toggleSelection(f.path)}
                        class="text-accent focus:outline-none flex-shrink-0 cursor-pointer"
                      >
                        {#if selectedPaths[f.path]}
                          <CheckSquare class="w-4.5 h-4.5 text-accent" />
                        {:else}
                          <Square class="w-4.5 h-4.5 text-text-muted group-hover:text-text-secondary transition-colors" />
                        {/if}
                      </button>
                    </td>

                    <!-- Project Name -->
                    <td class="py-3.5 px-5">
                      <span class="font-bold text-text-primary font-sans">{f.project_name}</span>
                    </td>

                    <!-- Clean Target Badge -->
                    <td class="py-3.5 px-5 font-mono text-xs">
                      <span class="px-2 py-0.5 rounded border text-[10px] font-semibold {meta.color}">
                        {f.name} ({meta.badge})
                      </span>
                    </td>

                    <!-- Size / Count -->
                    <td class="py-3.5 px-5">
                      <div class="flex flex-col font-mono text-xs">
                        <span class="font-bold text-accent">{formatSize(f.size_bytes)}</span>
                        <span class="text-[9px] text-text-muted">{t("{count} files", { count: f.file_count.toLocaleString() })}</span>
                      </div>
                    </td>

                    <!-- Last Modified -->
                    <td class="py-3.5 px-5 font-mono text-xs text-text-secondary">
                      {f.last_modified}
                    </td>

                    <!-- Path -->
                    <td class="py-3.5 px-5 font-mono text-[10px] text-text-muted truncate max-w-[280px]" title={f.path}>
                      {f.path}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    {/if}
  </div>

  <!-- Floating Action Bar for Deletion -->
  {#if selectedCount > 0 && !scanning}
    <div class="fixed bottom-6 left-1/2 transform -translate-x-1/2 z-40 bg-surface-bg/85 backdrop-blur-xl border border-border-default py-4 px-6 rounded-xl flex items-center justify-between shadow-2xl gap-8 animate-fade-in max-w-2xl w-[90%]">
      <div class="flex flex-col font-sans">
        <span class="text-xs text-text-secondary font-medium">{t("Sweep Plan Configured")}</span>
        <span class="text-[10px] text-text-muted mt-0.5">
          {t("Will delete")} <b class="text-accent">{selectedCount}</b> {t("project directories and free up")} <b class="text-accent font-mono">{formatSize(totalSelectedSize)}</b>
        </span>
      </div>

      <div class="flex gap-3 flex-shrink-0">
        <button
          onclick={() => selectAll(false)}
          class="px-4 py-2 rounded-lg text-xs font-semibold bg-app-bg hover:bg-elevated-bg border border-border-default text-text-secondary hover:text-text-primary transition-all cursor-pointer"
        >
          {t("Cancel")}
        </button>
        <button
          onclick={purgeSelected}
          disabled={deleting}
          class="flex items-center gap-2 px-5 py-2 rounded-lg text-xs font-bold bg-danger-strong hover:bg-danger-strong/90 text-white active:scale-95 transition-all shadow-lg cursor-pointer disabled:opacity-50"
        >
          {#if deleting}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            {t("Purging...")}
          {:else}
            <Trash2 class="w-3.5 h-3.5" />
            {t("Purge Selected")}
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<!-- Deleting Progress Modal -->
{#if deleting}
  <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4 animate-fade-in">
    <div class="bg-surface-bg border border-border-default w-full max-w-md rounded-xl p-6 flex flex-col items-center text-center space-y-4 shadow-2xl animate-scale-up">
      <div class="w-12 h-12 rounded-full border-2 border-danger/20 border-t-danger animate-spin flex items-center justify-center">
        <Trash2 class="w-5 h-5 text-danger animate-pulse" />
      </div>
      <div>
        <h4 class="text-base font-bold text-text-primary">{t("Purging Project Directories")}</h4>
        <p class="text-xs text-text-muted mt-1 font-mono">
          {t("Deleting {current} / {total} items", { current: deleteProgress.current, total: deleteProgress.total })}
        </p>
      </div>
      <div class="w-full bg-app-bg h-1.5 rounded-full overflow-hidden border border-border-default">
        <div 
          class="h-full bg-danger transition-all duration-300 rounded-full" 
          style="width: {(deleteProgress.current / deleteProgress.total) * 100}%"
        ></div>
      </div>
      <div class="w-full text-[10px] font-mono text-text-muted bg-app-bg p-2.5 rounded-lg border border-border-default truncate">
        {deleteProgress.message}
      </div>
    </div>
  </div>
{/if}

<!-- Deletion Report Modal -->
{#if showReportModal}
  <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4 animate-fade-in">
    <div class="bg-surface-bg border border-border-default w-full max-w-2xl rounded-xl flex flex-col max-h-[80vh] shadow-2xl relative overflow-hidden animate-scale-up">
      <!-- Header -->
      <header class="p-6 border-b border-border-default flex items-center justify-between">
        <div>
          <h3 class="text-base font-bold text-text-primary">
            {t("Sweeper Deletion Report")}
          </h3>
          <p class="text-xs text-text-muted mt-1">{t("Review the final deletion results of selected directories.")}</p>
        </div>
        <button
          onclick={() => showReportModal = false}
          class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
        >
          <X class="w-4 h-4" />
        </button>
      </header>

      <!-- Body -->
      <div class="flex-1 overflow-y-auto p-6 space-y-3 font-mono text-xs">
        {#each Object.entries(deleteReport) as [path, status]}
          <div class="flex items-center justify-between p-2.5 rounded-lg border bg-app-bg border-border-default gap-3">
            <span class="truncate text-text-secondary select-all" title={path}>{path}</span>
            {#if status === "Deleted" || status === "DeletedAfterUnlock" || status === "ForceDeleted"}
              <span class="px-2 py-0.5 rounded bg-success/15 border border-success/30 text-success text-[10px] font-semibold font-mono">
                {status}
              </span>
            {:else if status === "ScheduledForReboot"}
              <span class="px-2 py-0.5 rounded bg-info/10 border border-info/25 text-info text-[10px] font-semibold font-mono">
                {t("Boot-Pending")}
              </span>
            {:else}
              <span class="px-2 py-0.5 rounded bg-danger/15 border border-danger/30 text-danger text-[10px] font-semibold font-mono" title={status}>
                {t("Failed")}
              </span>
            {/if}
          </div>
        {/each}
      </div>

      <!-- Footer -->
      <footer class="p-6 border-t border-border-default flex justify-end bg-sidebar-bg">
        <button
          onclick={() => showReportModal = false}
          class="px-5 py-2.5 rounded-lg text-xs font-semibold bg-accent-strong hover:bg-accent-strong/90 text-white active:scale-95 transition-all shadow cursor-pointer"
        >
          {t("Close Report")}
        </button>
      </footer>
    </div>
  </div>
{/if}
