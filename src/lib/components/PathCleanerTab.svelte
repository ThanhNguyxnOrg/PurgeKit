<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { ShieldAlert, Trash2, CheckCircle2, ShieldCheck, RefreshCw, FolderSearch, CheckSquare, Square } from "@lucide/svelte";

  interface PathEntry {
    value: string;
    is_valid: boolean;
    scope: string; // "User" | "System"
  }

  let envPaths = $state<PathEntry[]>([]);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let selectedPaths = $state<Record<string, boolean>>({});

  async function loadPaths() {
    isLoading = true;
    selectedPaths = {};
    try {
      envPaths = await invoke<PathEntry[]>("get_path_entries");
      // Select all broken paths by default for cleaning
      envPaths.forEach((entry) => {
        if (!entry.is_valid) {
          selectedPaths[entry.value] = true;
        }
      });
    } catch (e) {
      console.error("Failed to load PATH entries:", e);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadPaths();
  });

  function toggleSelection(value: string) {
    selectedPaths[value] = !selectedPaths[value];
  }

  async function cleanSelectedPaths() {
    const pathsToRemove = Object.keys(selectedPaths).filter((key) => selectedPaths[key]);
    if (pathsToRemove.length === 0) {
      alert("No paths selected for cleaning.");
      return;
    }

    isSaving = true;
    try {
      // Split remaining paths by scope to save back to Registry
      const userRemaining = envPaths
        .filter((entry) => entry.scope === "User" && !selectedPaths[entry.value])
        .map((entry) => entry.value);

      const systemRemaining = envPaths
        .filter((entry) => entry.scope === "System" && !selectedPaths[entry.value])
        .map((entry) => entry.value);

      // Save User scope
      await invoke("save_path_entries", {
        remainingValues: userRemaining,
        scope: "User"
      });

      // Save System scope (requires Admin, handled in backend error check)
      const hasSystemRemoval = envPaths.some((entry) => entry.scope === "System" && selectedPaths[entry.value]);
      if (hasSystemRemoval) {
        await invoke("save_path_entries", {
          remainingValues: systemRemaining,
          scope: "System"
        });
      }

      alert("Selected broken paths cleaned successfully!");
      await loadPaths();
    } catch (e: any) {
      alert(`Clean failed: ${e.toString()}\n\nNote: Cleaning 'System' PATH requires running PurgeKit as Administrator.`);
    } finally {
      isSaving = false;
    }
  }
</script>

<div class="flex-1 flex flex-col h-screen bg-app-bg text-text-primary">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        PATH Environment Cleaner
      </h2>
      <p class="text-xs text-text-muted mt-1">Scan and clean broken, dead, or duplicate paths in your Windows PATH variables to avoid environment conflicts</p>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={cleanSelectedPaths}
        disabled={isSaving || Object.keys(selectedPaths).filter(k => selectedPaths[k]).length === 0}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-danger hover:bg-danger/80 disabled:opacity-50 active:scale-95 transition-all text-white shadow"
      >
        <Trash2 class="w-3.5 h-3.5" />
        Clean Selected Paths
      </button>

      <button
        onclick={loadPaths}
        disabled={isLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 disabled:opacity-50 transition-all text-text-primary"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
        Scan PATH
      </button>
    </div>
  </header>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    <!-- Stats Banner -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-center gap-3">
        <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center text-accent">
          <FolderSearch class="w-5 h-5 text-accent" />
        </div>
        <div>
          <span class="block text-[10px] text-text-muted font-mono uppercase">Total Analyzed</span>
          <span class="text-lg font-bold text-text-primary">{envPaths.length} Entries</span>
        </div>
      </div>

      <div class="p-4 rounded-lg bg-danger/10 border border-danger/20 flex items-center gap-3">
        <div class="w-10 h-10 rounded-lg bg-danger/20 border border-danger/30 flex items-center justify-center text-danger animate-fade-in">
          <ShieldAlert class="w-5 h-5 text-danger" />
        </div>
        <div>
          <span class="block text-[10px] text-text-muted font-mono uppercase">Broken Paths</span>
          <span class="text-lg font-bold text-danger">{envPaths.filter(p => !p.is_valid).length} Detected</span>
        </div>
      </div>

      <div class="p-4 rounded-lg bg-success/10 border border-success/20 flex items-center gap-3">
        <div class="w-10 h-10 rounded-lg bg-success/20 border border-success/30 flex items-center justify-center text-success">
          <CheckCircle2 class="w-5 h-5 text-success" />
        </div>
        <div>
          <span class="block text-[10px] text-text-muted font-mono uppercase">Healthy Paths</span>
          <span class="text-lg font-bold text-success">{envPaths.filter(p => p.is_valid).length} Entries</span>
        </div>
      </div>
    </div>

    <!-- Active List -->
    <div class="space-y-4">
      <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider">Path Variables</h3>
      
      {#if isLoading}
        <div class="flex flex-col items-center justify-center py-12 gap-3">
          <RefreshCw class="w-7 h-7 text-accent animate-spin" />
          <span class="text-xs text-text-muted font-mono">Scanning Windows PATH directories...</span>
        </div>
      {:else if envPaths.length === 0}
        <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center">
          <span class="text-sm font-sans text-text-muted font-medium">No PATH values found!</span>
        </div>
      {:else}
        <div class="border border-border-default rounded-lg overflow-hidden bg-surface-bg/50">
          <table class="w-full text-left border-collapse">
            <thead>
              <tr class="bg-sidebar-bg border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
                <th class="py-4 px-5 w-10">Select</th>
                <th class="py-4 px-5">Variable Scope</th>
                <th class="py-4 px-5">Path Value</th>
                <th class="py-4 px-5">Status</th>
                <th class="py-4 px-5">Details / Reason</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border-default text-sm">
              {#each envPaths as p}
                <tr class="hover:bg-elevated-bg/50 transition-colors duration-150 group">
                  <!-- Checkbox -->
                  <td class="py-4 px-5">
                    <button
                      onclick={() => toggleSelection(p.value)}
                      class="text-accent focus:outline-none flex-shrink-0"
                    >
                      {#if selectedPaths[p.value]}
                        <CheckSquare class="w-4.5 h-4.5 text-accent" />
                      {:else}
                        <Square class="w-4.5 h-4.5 text-text-muted group-hover:text-text-secondary transition-colors" />
                      {/if}
                    </button>
                  </td>
                  
                  <!-- Scope -->
                  <td class="py-4 px-5 font-mono text-xs">
                    <span class="px-2 py-0.5 rounded border
                      {p.scope === 'System' ? 'bg-surface-bg text-text-secondary border-border-default' : 'bg-brand-purple/10 text-brand-purple border-brand-purple/20'}">
                      {p.scope}
                    </span>
                  </td>
                  
                  <!-- Value -->
                  <td class="py-4 px-5 font-mono text-xs text-text-secondary group-hover:text-text-primary break-all max-w-[400px]">
                    {p.value}
                  </td>
                  
                  <!-- Status -->
                  <td class="py-4 px-5">
                    {#if !p.is_valid}
                      <span class="flex items-center gap-1.5 text-xs text-danger font-medium">
                        <ShieldAlert class="w-4 h-4 flex-shrink-0 text-danger" />
                        Broken
                      </span>
                    {:else}
                      <span class="flex items-center gap-1.5 text-xs text-success font-medium">
                        <CheckCircle2 class="w-4 h-4 flex-shrink-0 text-success" />
                        Healthy
                      </span>
                    {/if}
                  </td>
                  
                  <!-- Reason -->
                  <td class="py-4 px-5 text-xs text-text-muted font-sans">
                    {!p.is_valid ? "Directory does not exist on this machine" : "Valid directory"}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>
</div>
