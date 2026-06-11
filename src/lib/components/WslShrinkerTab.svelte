<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { 
    Cpu, RefreshCw, HardDrive, Trash2, ShieldAlert, ShieldCheck, 
    Terminal, Settings, ToggleLeft, ToggleRight, X, Play, Info
  } from "@lucide/svelte";
  import { toast } from "../toast.svelte";

  interface WslDistroInfo {
    id: string;
    name: string;
    base_path: string;
    vhdx_path: string | null;
    vhdx_size_bytes: number;
    is_sparse: boolean;
  }

  interface WslProgressPayload {
    phase: string;
    message: string;
  }

  let distros = $state<WslDistroInfo[]>([]);
  let isLoading = $state(true);
  let isAdmin = $state(false);

  let compactingDistro = $state<WslDistroInfo | null>(null);
  let compactPhase = $state("");
  let compactLog = $state("");
  let unlistenProgress: UnlistenFn | null = null;

  let updatingSparse = $state<Record<string, boolean>>({});

  async function loadDistros() {
    isLoading = true;
    try {
      distros = await invoke<WslDistroInfo[]>("get_wsl_distros");
      isAdmin = await invoke<boolean>("check_is_admin");
    } catch (e: any) {
      console.error(e);
      toast.show(`Failed to load WSL distros: ${e.toString()}`, "error");
    } finally {
      isLoading = false;
    }
  }

  async function toggleSparseMode(distro: WslDistroInfo, val: boolean) {
    updatingSparse[distro.name] = true;
    const actionText = val ? "Enable Auto-Shrink (Sparse)" : "Disable Auto-Shrink (Thick)";
    
    try {
      toast.show(`${actionText} for ${distro.name}...`, "info");
      await invoke("set_wsl_distro_sparse_mode", { name: distro.name, sparse: val });
      toast.show(`Successfully updated sparse property for ${distro.name}!`, "success");
      await loadDistros();
    } catch (e: any) {
      console.error(e);
      toast.show(`Failed to update sparse mode: ${e.toString()}`, "error");
    } finally {
      updatingSparse[distro.name] = false;
    }
  }

  async function compactDistro(distro: WslDistroInfo) {
    if (!isAdmin) {
      toast.show("WSL VHDX compaction requires Administrator privileges. Please run PurgeKit as Administrator.", "error");
      return;
    }

    if (!distro.vhdx_path) {
      toast.show("Could not locate the VHDX file path for this distribution.", "error");
      return;
    }

    const message = `Are you sure you want to compact the virtual disk for ${distro.name}?\n\nThis will temporarily shutdown WSL and use the Windows diskpart utility to reclaim deleted storage.`;
    if (!confirm(message)) {
      return;
    }

    compactingDistro = distro;
    compactPhase = "Shutting down WSL...";
    compactLog = "Initializing VHDX Compaction sequence...\n";

    try {
      unlistenProgress = await listen<WslProgressPayload>("wsl-compact-progress", (event) => {
        const payload = event.payload;
        if (payload.phase === "shutdown") {
          compactPhase = "Shutting down WSL...";
          compactLog += `[WSL] ${payload.message}\n`;
        } else if (payload.phase === "compacting") {
          compactPhase = "Compacting Virtual Disk...";
          compactLog += `[DiskPart] ${payload.message}\n`;
        } else if (payload.phase === "log") {
          compactLog += `[DiskPart Log]\n${payload.message}\n`;
        } else if (payload.phase === "completed") {
          compactPhase = "Completed";
          compactLog += `[Success] ${payload.message}\n`;
        } else if (payload.phase === "error") {
          compactPhase = "Failed";
          compactLog += `[Error] ${payload.message}\n`;
        }
      });

      await invoke<string>("compact_wsl_distro", {
        name: distro.name,
        vhdxPath: distro.vhdx_path
      });

      toast.show(`Successfully compacted virtual disk for ${distro.name}!`, "success");
      await loadDistros();
    } catch (e: any) {
      console.error(e);
      toast.show(`Compaction failed: ${e.toString()}`, "error");
    } finally {
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
    }
  }

  function closeCompactModal() {
    compactingDistro = null;
    compactLog = "";
    compactPhase = "";
  }

  // Helper size formats
  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes === undefined) return "0 Bytes";
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Derived calculations
  let totalDiskBytes = $derived(
    distros.reduce((acc, curr) => acc + (curr.vhdx_size_bytes || 0), 0)
  );

  onMount(() => {
    loadDistros();
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
  });
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <!-- Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        WSL2 Disk Shrinker
      </h2>
      <p class="text-xs text-text-muted mt-1">
        Reclaim physical hard disk space from bloated WSL2 virtual drive files (`ext4.vhdx`).
      </p>
    </div>

    <button
      onclick={loadDistros}
      disabled={isLoading}
      class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 disabled:opacity-50 transition-all text-text-primary cursor-pointer"
    >
      <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
      Refresh Distros
    </button>
  </header>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    
    <!-- Stats Row -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <!-- Total Space Card -->
      <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
        <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center text-accent">
          <HardDrive class="w-5 h-5" />
        </div>
        <div>
          <span class="block text-[10px] text-text-muted font-mono uppercase">Total VHDX Disk Size</span>
          <span class="text-lg font-bold text-text-primary">{formatSize(totalDiskBytes)}</span>
        </div>
      </div>

      <!-- Total Distros Card -->
      <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
        <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center text-text-secondary">
          <Cpu class="w-5 h-5" />
        </div>
        <div>
          <span class="block text-[10px] text-text-muted font-mono uppercase">WSL Distributions</span>
          <span class="text-lg font-bold text-text-primary">{distros.length} Detected</span>
        </div>
      </div>

      <!-- Admin Privilege Card -->
      {#if isAdmin}
        <div class="p-4 rounded-lg bg-success/10 border border-success/20 flex items-center gap-3 animate-fade-in">
          <div class="w-10 h-10 rounded-lg bg-success/20 border border-success/30 flex items-center justify-center text-success">
            <ShieldCheck class="w-5 h-5" />
          </div>
          <div>
            <span class="block text-[10px] text-success/80 font-mono uppercase">Privileges</span>
            <span class="text-xs font-bold text-success">Privileged Mode Active</span>
          </div>
        </div>
      {:else}
        <div class="p-4 rounded-lg bg-danger/10 border border-danger/20 flex items-center gap-3 animate-fade-in">
          <div class="w-10 h-10 rounded-lg bg-danger/20 border border-danger/30 flex items-center justify-center text-danger animate-pulse">
            <ShieldAlert class="w-5 h-5" />
          </div>
          <div>
            <span class="block text-[10px] text-danger/80 font-mono uppercase">Privileges Restricted</span>
            <span class="text-xs font-bold text-danger">Standard User (Run as Admin to Compact)</span>
          </div>
        </div>
      {/if}
    </div>

    <!-- Info Box -->
    <div class="p-4 border border-border-default rounded-xl bg-surface-bg/30 text-xs text-text-secondary leading-relaxed flex gap-3">
      <Info class="w-5 h-5 text-accent flex-shrink-0 mt-0.5" />
      <div class="space-y-1">
        <span class="font-bold text-text-primary block">Why do WSL2 virtual drives expand?</span>
        <p>
          WSL2 virtual drives (`ext4.vhdx`) grow as you write files inside Linux. However, when you delete files in Linux, the Windows host does not automatically shrink the `.vhdx` file. This utility allows you to manually co-root (compact) the virtual drives to reclaim unused storage or set them to sparse mode so Windows can automatically free up blocks in the background.
        </p>
      </div>
    </div>

    <!-- Active List -->
    <div class="space-y-4">
      <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider">WSL2 Distributions</h3>

      {#if isLoading}
        <div class="flex flex-col items-center justify-center py-16 gap-3">
          <RefreshCw class="w-7 h-7 text-accent animate-spin" />
          <span class="text-xs text-text-muted font-mono">Analyzing Windows Subsystem for Linux...</span>
        </div>
      {:else if distros.length === 0}
        <div class="py-16 text-center border border-dashed border-border-default rounded-xl bg-surface-bg/10 flex flex-col items-center justify-center space-y-2">
          <div class="w-10 h-10 rounded-lg bg-surface-bg border border-border-default flex items-center justify-center text-text-muted">
            <Cpu class="w-5.5 h-5.5" />
          </div>
          <h4 class="text-xs font-bold text-text-primary">No WSL2 distributions registered.</h4>
          <p class="text-[11px] text-text-muted">We could not find any active WSL configurations under HKCU registry.</p>
        </div>
      {:else}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          {#each distros as distro}
            <div class="border border-border-default rounded-xl p-5 bg-surface-bg/30 hover:border-border-strong transition-all flex flex-col justify-between space-y-5 relative overflow-hidden group">
              <!-- Background hint -->
              <div class="absolute -right-6 -bottom-6 text-text-disabled opacity-5 select-none pointer-events-none transition-transform duration-300 group-hover:scale-110">
                <Cpu class="w-24 h-24" />
              </div>

              <!-- Top Row Info -->
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0">
                  <h4 class="text-base font-bold text-text-primary truncate" title={distro.name}>
                    {distro.name}
                  </h4>
                  <span class="text-[10px] text-text-muted font-mono bg-app-bg px-1.5 py-0.5 rounded border border-border-default truncate block w-max mt-1 max-w-full" title={distro.id}>
                    ID: {distro.id}
                  </span>
                </div>

                <div class="text-right">
                  <span class="text-base font-extrabold font-mono text-accent">
                    {formatSize(distro.vhdx_size_bytes)}
                  </span>
                  <span class="block text-[9px] uppercase font-semibold text-text-muted mt-0.5 font-mono">
                    Drive size
                  </span>
                </div>
              </div>

              <!-- Disk Path Info -->
              <div class="space-y-1">
                <span class="text-[9px] text-text-muted font-mono uppercase tracking-wider block">VHDX Virtual Disk Path</span>
                {#if distro.vhdx_path}
                  <div class="flex items-center gap-1.5 bg-app-bg border border-border-default p-2 rounded-lg text-[10px] font-mono select-all truncate text-text-secondary" title={distro.vhdx_path}>
                    <HardDrive class="w-3.5 h-3.5 text-text-muted flex-shrink-0" />
                    <span class="truncate">{distro.vhdx_path}</span>
                  </div>
                {:else}
                  <div class="flex items-center gap-1.5 bg-danger/10 border border-danger/20 p-2 rounded-lg text-[10px] font-sans text-danger font-medium">
                    <ShieldAlert class="w-3.5 h-3.5 flex-shrink-0" />
                    <span>Unable to locate ext4.vhdx disk file.</span>
                  </div>
                {/if}
              </div>

              <!-- Actions Area -->
              <div class="pt-4 border-t border-border-default flex items-center justify-between gap-4">
                <!-- Sparse Mode toggle -->
                <div class="flex items-center gap-2">
                  <button
                    onclick={() => toggleSparseMode(distro, !distro.is_sparse)}
                    disabled={updatingSparse[distro.name] || !distro.vhdx_path}
                    class="focus:outline-none transition-all duration-150 disabled:opacity-50 flex items-center cursor-pointer"
                    title={distro.is_sparse ? "Disable Sparse Mode" : "Enable Sparse Mode"}
                  >
                    {#if distro.is_sparse}
                      <ToggleRight class="w-8 h-8 text-success" />
                    {:else}
                      <ToggleLeft class="w-8 h-8 text-text-muted" />
                    {/if}
                  </button>
                  <div class="flex flex-col text-left">
                    <span class="text-[11px] font-bold text-text-primary leading-none">Auto-Shrink</span>
                    <span class="text-[9px] text-text-muted mt-0.5">Sparse VHDX</span>
                  </div>
                </div>

                <!-- Compact Button -->
                <button
                  onclick={() => compactDistro(distro)}
                  disabled={!distro.vhdx_path || updatingSparse[distro.name]}
                  class="flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-bold transition-all border shadow-sm cursor-pointer disabled:opacity-40
                    {!isAdmin 
                      ? 'bg-surface-bg border-border-default text-text-secondary hover:bg-elevated-bg' 
                      : 'bg-accent hover:bg-accent-hover text-white border-accent'}"
                  title={!isAdmin ? "Requires Administrator privileges" : "Compact disk now"}
                >
                  <Trash2 class="w-3.5 h-3.5" />
                  Compact Disk
                </button>
              </div>

            </div>
          {/each}
        </div>
      {/if}
    </div>

  </div>
</div>

<!-- Compact Console Modal -->
{#if compactingDistro}
  <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4 animate-fade-in">
    <div class="bg-surface-bg border border-border-default w-full max-w-2xl rounded-xl flex flex-col max-h-[85vh] shadow-2xl relative overflow-hidden animate-scale-up">
      <!-- Header -->
      <header class="p-6 border-b border-border-default flex items-center justify-between">
        <div>
          <h3 class="text-base font-bold text-text-primary flex items-center gap-2">
            <Terminal class="w-4.5 h-4.5 text-accent animate-pulse" />
            VHDX Compaction Sequence: <span class="text-accent">{compactingDistro.name}</span>
          </h3>
          <p class="text-xs text-text-muted mt-1">Status: <span class="text-accent font-semibold">{compactPhase}</span></p>
        </div>
        
        {#if compactPhase === "Completed" || compactPhase === "Failed"}
          <button
            onclick={closeCompactModal}
            class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
          >
            <X class="w-4 h-4" />
          </button>
        {/if}
      </header>

      <!-- Terminal Body -->
      <div class="flex-1 overflow-y-auto p-6 bg-black/90 font-mono text-[11px] text-green-400 leading-relaxed min-h-[200px] border-b border-border-default selection:bg-green-800/50 selection:text-white select-text">
        <pre class="whitespace-pre-wrap">{compactLog}</pre>
        
        {#if compactPhase !== "Completed" && compactPhase !== "Failed"}
          <div class="flex items-center gap-2 mt-4 text-accent animate-pulse">
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Processing virtual hard disk compaction...</span>
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <footer class="p-6 border-t border-border-default flex justify-end bg-sidebar-bg">
        <button
          onclick={closeCompactModal}
          disabled={compactPhase !== "Completed" && compactPhase !== "Failed"}
          class="px-5 py-2.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white active:scale-95 disabled:opacity-50 transition-all shadow cursor-pointer"
        >
          {compactPhase === "Completed" || compactPhase === "Failed" ? "Close Terminal" : "Compacting Disk..."}
        </button>
      </footer>
    </div>
  </div>
{/if}
