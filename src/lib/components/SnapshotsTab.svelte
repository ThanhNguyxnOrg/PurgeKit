<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Camera, Calendar, ArrowRight, Shield, Layers, RefreshCw, Database, FileText, CheckCircle, Trash2, X } from "@lucide/svelte";

  interface SnapshotRecord {
    id: string;
    name: string;
    created_at: string;
    data_file_path: string;
    reg_count: number;
    file_count: number;
  }

  interface SnapshotDiff {
    new_registry_keys: string[];
    new_files: string[];
  }

  let snapshots = $state<SnapshotRecord[]>([]);
  let isLoading = $state(true);
  let isCreating = $state(false);
  let newSnapshotName = $state("");
  let showCreateModal = $state(false);

  // Compare States
  let selectedBefore = $state("");
  let selectedAfter = $state("");
  let diffResults = $state<SnapshotDiff | null>(null);
  let isComparing = $state(false);

  async function loadSnapshots() {
    isLoading = true;
    try {
      snapshots = await invoke<SnapshotRecord[]>("list_snapshots");
    } catch (e) {
      console.error("Failed to load snapshots:", e);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadSnapshots();
  });

  async function handleCreateSnapshot() {
    if (newSnapshotName.trim() === "") {
      alert("Please enter a name for the snapshot.");
      return;
    }

    isCreating = true;
    try {
      await invoke("take_snapshot", { name: newSnapshotName });
      newSnapshotName = "";
      showCreateModal = false;
      await loadSnapshots();
    } catch (e: any) {
      alert(`Failed to create snapshot: ${e.toString()}`);
    } finally {
      isCreating = false;
    }
  }

  async function handleCompare() {
    if (!selectedBefore || !selectedAfter) {
      alert("Please select both a BEFORE and AFTER snapshot to compare.");
      return;
    }

    if (selectedBefore === selectedAfter) {
      alert("Please select two different snapshots to compare.");
      return;
    }

    isComparing = true;
    diffResults = null;
    try {
      diffResults = await invoke<SnapshotDiff>("compare_snapshots", {
        beforeId: selectedBefore,
        afterId: selectedAfter,
      });
    } catch (e: any) {
      alert(`Comparison failed: ${e.toString()}`);
    } finally {
      isComparing = false;
    }
  }

  async function handleDeleteSnapshot(id: string) {
    if (!confirm("Are you sure you want to delete this snapshot?")) return;
    try {
      await invoke("delete_snapshot", { id });
      if (selectedBefore === id) selectedBefore = "";
      if (selectedAfter === id) selectedAfter = "";
      await loadSnapshots();
    } catch (e: any) {
      alert(`Failed to delete snapshot: ${e.toString()}`);
    }
  }

  function clearCompare() {
    selectedBefore = "";
    selectedAfter = "";
    diffResults = null;
  }

  // Reactive state helper for stepper indicators
  let step1Status = $derived(snapshots.length > 0 ? 'completed' : 'active');
  let step2Status = $derived(snapshots.length > 0 && !diffResults ? 'active' : (diffResults ? 'completed' : 'pending'));
  let step3Status = $derived(diffResults ? 'completed' : (selectedBefore && selectedAfter ? 'active' : 'pending'));
</script>

<div class="flex-1 flex flex-col h-screen bg-app-bg text-text-primary relative">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        System Snapshots
      </h2>
      <p class="text-xs text-text-muted mt-1">Track system changes by comparing snapshots taken before and after app installations for 100% clean uninstall</p>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={() => showCreateModal = true}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white active:scale-95 transition-all shadow"
      >
        <Camera class="w-3.5 h-3.5" />
        Create Snapshot
      </button>
      <button
        onclick={loadSnapshots}
        disabled={isLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 disabled:opacity-50 transition-all text-text-primary"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
        Refresh
      </button>
    </div>
  </header>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    <!-- Notice Banner -->
    <div class="p-4 rounded-lg bg-surface-bg border border-border-default flex items-start gap-4">
      <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center flex-shrink-0 text-accent">
        <Shield class="w-5 h-5 text-accent" />
      </div>
      <div>
        <h3 class="text-sm font-bold text-text-primary">How Active Monitoring Works</h3>
        <p class="text-xs text-text-muted mt-1 leading-relaxed">
          1. Take a snapshot <b>before</b> installing a new application.<br>
          2. Install the application and run it once.<br>
          3. Take a snapshot <b>after</b> installation. PurgeKit compares them, tracking every registry entry and file created, guaranteeing 100% trace-free removal.
        </p>
      </div>
    </div>

    <!-- Workflow Stepper -->
    <div class="grid grid-cols-1 md:grid-cols-5 items-center gap-4 p-5 rounded-lg bg-surface-bg border border-border-default">
      <div class="flex items-center gap-3 md:col-span-1">
        <div class="w-7 h-7 rounded-full flex items-center justify-center text-xs font-mono font-bold border
          {step1Status === 'completed' ? 'bg-success/10 text-success border-success/20' : 'bg-accent-subtle text-accent border-accent/20'}">
          1
        </div>
        <div class="flex flex-col">
          <span class="text-xs font-bold {step1Status === 'completed' ? 'text-success' : 'text-text-primary'}">Take Baseline</span>
          <span class="text-[10px] text-text-muted">Scan before install</span>
        </div>
      </div>
      
      <div class="hidden md:block h-px bg-border-default md:col-span-1"></div>
      
      <div class="flex items-center gap-3 md:col-span-1">
        <div class="w-7 h-7 rounded-full flex items-center justify-center text-xs font-mono font-bold border
          {step2Status === 'completed' ? 'bg-success/10 text-success border-success/20' : (step2Status === 'active' ? 'bg-accent-subtle text-accent border-accent/20' : 'bg-app-bg text-text-disabled border-border-default')}">
          2
        </div>
        <div class="flex flex-col">
          <span class="text-xs font-bold {step2Status === 'completed' ? 'text-success' : (step2Status === 'active' ? 'text-text-primary' : 'text-text-disabled')}">Install Software</span>
          <span class="text-[10px] text-text-muted font-normal">Run app installer</span>
        </div>
      </div>

      <div class="hidden md:block h-px bg-border-default md:col-span-1"></div>

      <div class="flex items-center gap-3 md:col-span-1">
        <div class="w-7 h-7 rounded-full flex items-center justify-center text-xs font-mono font-bold border
          {step3Status === 'completed' ? 'bg-success/10 text-success border-success/20' : (step3Status === 'active' ? 'bg-accent-subtle text-accent border-accent/20' : 'bg-app-bg text-text-disabled border-border-default')}">
          3
        </div>
        <div class="flex flex-col">
          <span class="text-xs font-bold {step3Status === 'completed' ? 'text-success' : (step3Status === 'active' ? 'text-text-primary' : 'text-text-disabled')}">Compare Changes</span>
          <span class="text-[10px] text-text-muted">Generate clean diff</span>
        </div>
      </div>
    </div>

    <!-- Main Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
      <!-- Snapshots List (2 Cols on lg) -->
      <div class="lg:col-span-2 space-y-4">
        <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider">Saved System Snapshots</h3>
        
        {#if isLoading}
          <div class="flex flex-col items-center justify-center py-12 gap-3">
            <RefreshCw class="w-7 h-7 text-accent animate-spin" />
            <span class="text-xs text-text-muted font-mono">Loading saved snapshots...</span>
          </div>
        {:else if snapshots.length === 0}
          <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center">
            <span class="text-sm font-sans text-text-muted font-medium">No Snapshots Found</span>
            <span class="text-xs text-text-muted mt-1">Take a snapshot before installing apps to start tracking.</span>
          </div>
        {:else}
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            {#each snapshots as snap}
              <div class="border border-border-default rounded-lg p-5 bg-surface-bg/50 hover:border-border-strong transition-all duration-200 flex flex-col relative group">
                <div class="flex items-start justify-between">
                  <div class="flex items-center gap-3">
                    <div class="w-9 h-9 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
                      <Layers class="w-4 h-4 text-accent" />
                    </div>
                    <div>
                      <h4 class="text-sm font-bold font-sans text-text-primary group-hover:text-white transition-colors">{snap.name}</h4>
                      <div class="flex items-center gap-1.5 text-[10px] text-text-muted font-mono mt-0.5">
                        <Calendar class="w-3 h-3" />
                        <span class="truncate max-w-[150px]">{snap.created_at}</span>
                      </div>
                    </div>
                  </div>
                  <button 
                    onclick={() => handleDeleteSnapshot(snap.id)}
                    class="p-1 rounded bg-transparent text-text-muted hover:text-danger hover:bg-danger/10 border border-transparent hover:border-danger/20 transition-all opacity-0 group-hover:opacity-100"
                    title="Delete snapshot"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </div>

                <!-- Stats -->
                <div class="grid grid-cols-2 gap-2 mt-4 p-3 rounded-lg bg-app-bg border border-border-default text-center font-mono text-[10px]">
                  <div>
                    <span class="block text-text-muted">Registry Keys</span>
                    <span class="text-xs font-bold text-text-primary mt-0.5 block">{snap.reg_count}</span>
                  </div>
                  <div class="border-l border-border-default">
                    <span class="block text-text-muted">Files Tracked</span>
                    <span class="text-xs font-bold text-text-primary mt-0.5 block">{snap.file_count}</span>
                  </div>
                </div>

                <!-- Scope Selection Indicators -->
                <div class="mt-4 pt-3 border-t border-border-default flex justify-between gap-2">
                  <button
                    onclick={() => selectedBefore = snap.id}
                    class="flex-1 py-1.5 rounded-md text-[10px] font-semibold transition-all border
                      {selectedBefore === snap.id 
                        ? 'bg-accent-subtle text-accent border-accent/20' 
                        : 'bg-surface-bg text-text-secondary border-border-default hover:text-text-primary hover:bg-elevated-bg'}"
                  >
                    Set BEFORE
                  </button>
                  <button
                    onclick={() => selectedAfter = snap.id}
                    class="flex-1 py-1.5 rounded-md text-[10px] font-semibold transition-all border
                      {selectedAfter === snap.id 
                        ? 'bg-accent-subtle text-accent border-accent/20' 
                        : 'bg-surface-bg text-text-secondary border-border-default hover:text-text-primary hover:bg-elevated-bg'}"
                  >
                    Set AFTER
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Compare Sidebar (1 Col on lg) -->
      <div class="border border-border-default rounded-lg p-5 bg-surface-bg/50 space-y-4">
        <h3 class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider">Compare snapshots</h3>

        <div class="space-y-3">
          <div class="space-y-1">
            <span class="text-[10px] text-text-muted uppercase font-mono">Snapshot BEFORE installation</span>
            <div class="p-3 rounded-lg bg-app-bg border border-border-default font-sans text-xs text-text-secondary truncate">
              {snapshots.find(s => s.id === selectedBefore)?.name || "Select a snapshot below..."}
            </div>
          </div>

          <div class="space-y-1">
            <span class="text-[10px] text-text-muted uppercase font-mono">Snapshot AFTER installation</span>
            <div class="p-3 rounded-lg bg-app-bg border border-border-default font-sans text-xs text-text-secondary truncate">
              {snapshots.find(s => s.id === selectedAfter)?.name || "Select a snapshot below..."}
            </div>
          </div>
        </div>

        <div class="flex gap-2 pt-2">
          {#if selectedBefore || selectedAfter}
            <button
              onclick={clearCompare}
              class="flex-1 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all"
            >
              Reset
            </button>
          {/if}
          <button
            onclick={handleCompare}
            disabled={isComparing || !selectedBefore || !selectedAfter || selectedBefore === selectedAfter}
            class="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow"
          >
            {#if isComparing}
              <RefreshCw class="w-3.5 h-3.5 animate-spin" />
              Comparing...
            {:else}
              Compare
              <ArrowRight class="w-3.5 h-3.5" />
            {/if}
          </button>
        </div>
      </div>
    </div>

    <!-- Diff Results Panel -->
    {#if diffResults}
      <div class="border border-border-default rounded-lg p-6 bg-surface-bg/30 space-y-4 animate-fade-in">
        <h3 class="text-sm font-bold text-text-primary flex items-center gap-2">
          <Database class="w-4.5 h-4.5 text-accent" />
          Comparison Results ({diffResults.new_registry_keys.length + diffResults.new_files.length} differences found)
        </h3>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          <!-- Registry New Keys -->
          <div class="space-y-3">
            <span class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider flex items-center gap-1.5">
              <Database class="w-3.5 h-3.5 text-brand-purple" />
              New Registry Keys ({diffResults.new_registry_keys.length})
            </span>
            <div class="border border-border-default rounded-lg bg-app-bg p-4 max-h-[300px] overflow-y-auto font-mono text-[10px] space-y-1.5">
              {#if diffResults.new_registry_keys.length === 0}
                <span class="text-text-muted italic block py-4 text-center">No new registry keys detected.</span>
              {:else}
                {#each diffResults.new_registry_keys as key}
                  <span class="block text-success break-all select-all">+ {key}</span>
                {/each}
              {/if}
            </div>
          </div>

          <!-- Files New Paths -->
          <div class="space-y-3">
            <span class="text-xs font-mono font-semibold uppercase text-text-muted tracking-wider flex items-center gap-1.5">
              <FileText class="w-3.5 h-3.5 text-info" />
              New Files & Folders ({diffResults.new_files.length})
            </span>
            <div class="border border-border-default rounded-lg bg-app-bg p-4 max-h-[300px] overflow-y-auto font-mono text-[10px] space-y-1.5">
              {#if diffResults.new_files.length === 0}
                <span class="text-text-muted italic block py-4 text-center">No new files detected.</span>
              {:else}
                {#each diffResults.new_files as file}
                  <span class="block text-success break-all select-all">+ {file}</span>
                {/each}
              {/if}
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Create Snapshot Modal Overlay -->
  {#if showCreateModal}
    <div class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4">
      <div class="bg-surface-bg border border-border-default w-full max-w-md rounded-lg flex flex-col shadow-2xl p-6 relative animate-scale-up duration-200">
        <h3 class="text-base font-bold text-text-primary">Create System Snapshot</h3>
        <p class="text-xs text-text-muted mt-1">Take a scan of important registry directories & application folders to establish a before/after baseline.</p>
        
        <div class="mt-4 space-y-2">
          <label for="snap-name-input" class="text-xs font-medium text-text-secondary">Snapshot Name</label>
          <input
            type="text"
            id="snap-name-input"
            placeholder="e.g., Before Docker Install"
            bind:value={newSnapshotName}
            class="w-full px-3.5 py-2 rounded-lg bg-app-bg border border-border-default focus:border-accent/50 outline-none text-xs font-sans text-text-primary transition-all placeholder:text-text-muted"
          />
        </div>

        <div class="mt-6 flex justify-end gap-3">
          <button
            onclick={() => { showCreateModal = false; newSnapshotName = ""; }}
            class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all"
          >
            Cancel
          </button>
          <button
            onclick={handleCreateSnapshot}
            disabled={isCreating || !newSnapshotName.trim()}
            class="flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow"
          >
            {#if isCreating}
              <RefreshCw class="w-3.5 h-3.5 animate-spin" />
              Scanning System...
            {:else}
              Scan Baseline
            {/if}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
