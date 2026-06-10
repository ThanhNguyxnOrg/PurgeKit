<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Terminal, RefreshCw, Trash2, HardDrive, AlertCircle, CheckCircle, FolderOpen } from "@lucide/svelte";

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

  let tools = $state<DevToolInfo[]>([]);
  let pathIssuesCount = $state(0);
  let isLoading = $state(true);
  let cleaningStates = $state<Record<string, boolean>>({});
  let message = $state<{ type: "success" | "error"; text: string } | null>(null);

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

  onMount(() => {
    loadDevTools();
  });

  // Helper to format file size
  function formatSize(bytes: number | null): string {
    if (bytes === null) return "N/A";
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Handle clean single dev cache
  async function cleanCache(name: string) {
    cleaningStates[name] = true;
    message = null;
    try {
      const freedBytes = await invoke<number>("clean_dev_tool_cache", { name });
      message = {
        type: "success",
        text: `Successfully cleaned cache for ${name}. Freed ${formatSize(freedBytes)}!`,
      };
      // Reload dev tools info to reflect changes
      await loadDevTools();
    } catch (err: any) {
      message = {
        type: "error",
        text: `Failed to clean ${name} cache: ${err.toString()}`,
      };
    } finally {
      cleaningStates[name] = false;
    }
  }

  // Clean all detected caches
  async function cleanAllCaches() {
    const detectedTools = tools.filter(t => t.detected && t.cache_size && t.cache_size > 0);
    if (detectedTools.length === 0) {
      message = { type: "success", text: "All caches are already clean!" };
      return;
    }

    message = null;
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
      message = {
        type: "error",
        text: `Freed ${formatSize(totalFreed)} but failed on: ${failedTools.join(", ")}`,
      };
    } else {
      message = {
        type: "success",
        text: `Successfully cleaned all caches! Total freed: ${formatSize(totalFreed)}`,
      };
    }
  }

  let totalCacheSize = $derived(
    tools.filter(t => t.detected).reduce((acc, curr) => acc + (curr.cache_size || 0), 0)
  );

  let detectedToolsCount = $derived(
    tools.filter(t => t.detected).length
  );
</script>

<div class="flex-1 flex flex-col h-screen bg-app-bg text-text-primary">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        Dev Tools
      </h2>
      <p class="text-xs text-text-muted mt-1">Free up storage by cleaning unused package caches & build caches from developer tools</p>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={cleanAllCaches}
        disabled={isLoading || tools.filter(t => t.detected && t.cache_size && t.cache_size > 0).length === 0}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow"
      >
        <Trash2 class="w-3.5 h-3.5" />
        Purge All Caches
      </button>

      <button
        onclick={loadDevTools}
        disabled={isLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-bg border border-border-default hover:bg-elevated-bg active:scale-95 disabled:opacity-50 transition-all text-text-primary"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
        Refresh
      </button>
    </div>
  </header>

  <!-- Info Messages -->
  {#if message}
    <div class="mx-6 mt-6 p-4 rounded-lg flex items-center gap-3 border animate-fade-in
      {message.type === 'success' ? 'bg-success/10 text-success border-success/20' : 'bg-danger/10 text-danger border-danger/20'}">
      {#if message.type === 'success'}
        <CheckCircle class="w-5 h-5 flex-shrink-0" />
      {:else}
        <AlertCircle class="w-5 h-5 flex-shrink-0" />
      {/if}
      <span class="text-sm font-sans">{message.text}</span>
    </div>
  {/if}

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    {#if isLoading}
      <div class="flex flex-col items-center justify-center h-64 gap-3">
        <RefreshCw class="w-8 h-8 text-accent animate-spin" />
        <span class="text-sm text-text-muted font-mono">Analyzing local developer caches...</span>
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
            <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">Total Cache</span>
            <span class="text-lg font-bold text-text-primary">{formatSize(totalCacheSize)}</span>
          </div>
        </div>

        <!-- Tools Found Card -->
        <div class="p-4 rounded-lg bg-surface-bg border-l-4 border-l-border-strong border border-border-default flex items-center gap-3 shadow-sm">
          <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center">
            <Terminal class="w-5 h-5 text-text-secondary" />
          </div>
          <div>
            <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">Tools Found</span>
            <span class="text-lg font-bold text-text-primary">{detectedToolsCount} Detected</span>
          </div>
        </div>

        <!-- PATH Issues Card -->
        <div class="p-4 rounded-lg bg-surface-bg border-l-4 border-l-danger border border-border-default flex items-center gap-3 shadow-sm">
          <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center animate-fade-in">
            <AlertCircle class="w-5 h-5 text-danger" />
          </div>
          <div>
            <span class="block text-[10px] text-text-muted font-mono uppercase tracking-wide">PATH Issues</span>
            <span class="text-lg font-bold text-danger">{pathIssuesCount} Broken</span>
          </div>
        </div>
      </div>

      <!-- Tools Grid -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {#each tools as tool}
          <div class="border rounded-lg p-5 flex flex-col bg-surface-bg/50 transition-all duration-200 relative overflow-hidden group
            {tool.detected 
              ? 'border-border-default hover:border-border-strong' 
              : 'border-border-subtle opacity-40 bg-surface-bg/10'}">
            
            <!-- Card Header -->
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 rounded-lg bg-app-bg border border-border-default flex items-center justify-center shadow-sm">
                  <Terminal class="w-5 h-5 {tool.detected ? 'text-accent' : 'text-text-disabled'}" />
                </div>
                <div>
                  <h3 class="text-base font-bold font-sans capitalize text-text-primary">{tool.name}</h3>
                  <span class="text-[10px] font-mono {tool.detected ? 'text-success' : 'text-text-muted'}">
                    {tool.detected ? `Version ${tool.version}` : 'Not Detected'}
                  </span>
                </div>
              </div>
              
              {#if tool.detected}
                <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-success/10 text-success border border-success/20">
                  Ready
                </span>
              {/if}
            </div>

            <!-- Path details -->
            <div class="mt-4 flex-1 space-y-2.5 text-xs">
              {#if tool.detected}
                <div class="space-y-1">
                  <span class="text-text-muted font-mono text-[9px] uppercase tracking-wide">Cache Path</span>
                  <div class="flex items-center gap-1.5 text-text-secondary bg-app-bg p-2 rounded-lg border border-border-default font-mono text-[10px] select-all truncate" title={tool.cache_path}>
                    <FolderOpen class="w-3.5 h-3.5 text-text-muted flex-shrink-0" />
                    <span class="truncate">{tool.cache_path}</span>
                  </div>
                </div>

                <div class="flex items-center justify-between bg-app-bg/50 p-3 rounded-lg border border-border-default">
                  <div class="flex items-center gap-2">
                    <HardDrive class="w-4 h-4 text-text-secondary" />
                    <span class="text-text-secondary font-sans">Current Cache Size</span>
                  </div>
                  <span class="text-sm font-bold font-mono text-accent">
                    {formatSize(tool.cache_size)}
                  </span>
                </div>
              {:else}
                <p class="text-text-muted italic py-6 text-center font-sans">
                  This tool was not found in your system's PATH. No caches to manage.
                </p>
              {/if}
            </div>

            <!-- Actions -->
            {#if tool.detected}
              <div class="mt-5 pt-4 border-t border-border-default flex gap-2">
                <button
                  onclick={() => cleanCache(tool.name)}
                  disabled={cleaningStates[tool.name] || !tool.cache_size || tool.cache_size === 0}
                  class="w-full flex items-center justify-center gap-2 py-2 rounded-lg text-xs font-semibold bg-surface-bg hover:bg-elevated-bg border border-border-default text-text-primary disabled:opacity-40 transition-all active:scale-[0.98]"
                >
                  {#if cleaningStates[tool.name]}
                    <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                    Cleaning...
                  {:else}
                    <Trash2 class="w-3.5 h-3.5" />
                    Purge Cache
                  {/if}
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
