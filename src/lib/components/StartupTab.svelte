<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Zap, RefreshCw, Trash2, Search, Check, X, ShieldAlert, Monitor, FileCode, CheckSquare, Square, AlertTriangle } from "@lucide/svelte";
  import { toast } from "../toast.svelte";
  import { t } from "$lib/i18n.svelte";

  interface StartupItem {
    id: string;
    name: string;
    command: string;
    location: string;
    enabled: boolean;
    is_registry: boolean;
    registry_path: string | null;
    file_path: string | null;
    icon_base64: string | null;
    target_missing: boolean;
  }

  let items = $state<StartupItem[]>([]);
  let isLoading = $state(true);
  let searchQuery = $state("");
  let filterType = $state<"all" | "registry" | "file">("all");
  let filterStatus = $state<"all" | "enabled" | "disabled">("all");
  let isAdmin = $state(false);

  async function loadItems() {
    isLoading = true;
    try {
      items = await invoke<StartupItem[]>("get_startup_items");
      isAdmin = await invoke<boolean>("check_is_admin");
    } catch (e) {
      console.error("Failed to load startup items:", e);
      toast.show(t("Error scanning startup programs!"), "error");
    } finally {
      isLoading = false;
    }
  }

  async function toggleStatus(item: StartupItem) {
    const originalStatus = item.enabled;
    const targetStatus = !item.enabled;
    
    // Optimistic UI update
    item.enabled = targetStatus;
    
    try {
      await invoke("set_startup_item_status", { id: item.id, enabled: targetStatus });
      toast.show(
        targetStatus
          ? t("Startup program enabled successfully!")
          : t("Startup program disabled successfully!"),
        "success"
      );
      // Reload to update the location labels (e.g. disabled subkey)
      await loadItems();
    } catch (e: any) {
      console.error("Failed to set startup item status:", e);
      // Revert status on failure
      item.enabled = originalStatus;
      toast.show(t("Operation failed: {error}", { error: e.toString() }), "error");
    }
  }

  async function deleteItem(item: StartupItem) {
    if (!confirm(t("Are you sure you want to permanently delete the startup item \"{name}\"?", { name: item.name }))) {
      return;
    }

    try {
      await invoke("delete_startup_item", { id: item.id });
      toast.show(t("Startup item \"{name}\" deleted successfully!", { name: item.name }), "success");
      await loadItems();
    } catch (e: any) {
      console.error("Failed to delete startup item:", e);
      toast.show(t("Failed to delete: {error}", { error: e.toString() }), "error");
    }
  }

  onMount(() => {
    loadItems();
  });

  // Filter and search logic
  let filteredItems = $derived(
    items.filter((item) => {
      const matchesSearch = item.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        item.command.toLowerCase().includes(searchQuery.toLowerCase()) ||
        item.location.toLowerCase().includes(searchQuery.toLowerCase());
      
      const matchesType = filterType === "all" ||
        (filterType === "registry" && item.is_registry) ||
        (filterType === "file" && !item.is_registry);

      const matchesStatus = filterStatus === "all" ||
        (filterStatus === "enabled" && item.enabled) ||
        (filterStatus === "disabled" && !item.enabled);

      return matchesSearch && matchesType && matchesStatus;
    })
  );

  function getAvatarStyle(name: string): string {
    const charCode = name.charCodeAt(0) || 0;
    const colors = [
      "bg-success/15 text-success border-success/25",
      "bg-info/15 text-info border-info/25",
      "bg-warning/15 text-warning border-warning/25",
      "bg-danger/15 text-danger border-danger/25",
      "bg-brand-purple/10 text-brand-purple border-brand-purple/20",
    ];
    return colors[charCode % colors.length];
  }
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        {t("Startup Manager")}
      </h2>
      <p class="text-xs text-text-muted mt-1 font-sans">
        {t("Manage programs that launch with Windows to optimize boot speed and system performance")}
      </p>
    </div>
    <button
      onclick={loadItems}
      disabled={isLoading}
      class="p-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg transition-colors flex items-center justify-center cursor-pointer disabled:opacity-50"
    >
      <RefreshCw class="w-4 h-4 text-text-secondary {isLoading ? 'animate-spin text-accent' : ''}" />
    </button>
  </header>

  <!-- Main Toolbar -->
  <div class="p-6 border-b border-border-default bg-sidebar-bg/50 flex flex-col md:flex-row md:items-center justify-between gap-4">
    <!-- Search -->
    <div class="relative max-w-sm w-full">
      <Search class="absolute left-3 top-2.5 w-4 h-4 text-text-muted" />
      <input
        type="text"
        placeholder={t("Search programs, commands...")}
        bind:value={searchQuery}
        class="w-full pl-9 pr-4 py-2 text-xs rounded-lg bg-app-bg border border-border-default text-text-primary placeholder:text-text-muted outline-none focus:border-accent/50"
      />
    </div>

    <!-- Filters -->
    <div class="flex flex-wrap items-center gap-3">
      <!-- Filter Type -->
      <div class="flex border border-border-default rounded-lg p-0.5 bg-app-bg">
        <button
          onclick={() => filterType = "all"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterType === 'all' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("All")}
        </button>
        <button
          onclick={() => filterType = "registry"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterType === 'registry' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("Registry")}
        </button>
        <button
          onclick={() => filterType = "file"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterType === 'file' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("Files")}
        </button>
      </div>

      <!-- Filter Status -->
      <div class="flex border border-border-default rounded-lg p-0.5 bg-app-bg">
        <button
          onclick={() => filterStatus = "all"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterStatus === 'all' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("All Statuses")}
        </button>
        <button
          onclick={() => filterStatus = "enabled"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterStatus === 'enabled' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("Enabled")}
        </button>
        <button
          onclick={() => filterStatus = "disabled"}
          class="px-3 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer {filterStatus === 'disabled' ? 'bg-surface-bg text-accent font-semibold shadow-sm' : 'text-text-muted hover:text-text-secondary'}"
        >
          {t("Disabled")}
        </button>
      </div>
    </div>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-6">
    {#if isLoading}
      <div class="w-full h-full flex flex-col items-center justify-center gap-3">
        <RefreshCw class="w-8 h-8 text-accent animate-spin" />
        <span class="text-xs text-text-muted font-sans">{t("Loading startup programs...")}</span>
      </div>
    {:else if filteredItems.length === 0}
      <div class="w-full h-full flex flex-col items-center justify-center text-center p-12 border border-dashed border-border-default rounded-xl bg-surface-bg/10">
        <Zap class="w-10 h-10 text-text-muted mb-3 animate-pulse" />
        <h3 class="text-sm font-bold text-text-primary">{t("No startup items found")}</h3>
        <p class="text-xs text-text-muted mt-1 max-w-xs leading-relaxed">
          {t("Try searching with a different keyword or changing filters.")}
        </p>
      </div>
    {:else}
      <div class="border border-border-default rounded-lg overflow-hidden bg-surface-bg/50">
        <div class="overflow-x-auto w-full">
          <table class="w-full table-fixed text-left border-collapse min-w-[720px]">
          <thead>
            <tr class="bg-sidebar-bg border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
              <th class="py-4 px-3 lg:px-5 w-[24%]">{t("Program Name")}</th>
              <th class="py-4 px-3 lg:px-5 w-[18%]">{t("Startup Location")}</th>
              <th class="py-4 px-3 lg:px-5 w-[30%]">{t("Command")}</th>
              <th class="py-4 px-3 lg:px-5 w-[16%]">{t("Status")}</th>
              <th class="py-4 px-3 lg:px-5 w-[12%] text-right">{t("Action")}</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border-default text-sm">
            {#each filteredItems as item}
              <tr class="hover:bg-elevated-bg/50 transition-colors duration-150 group">
                <!-- Name -->
                <td class="py-4 px-3 lg:px-5 font-sans font-medium flex items-center gap-3 min-w-0">
                  <div class="app-icon-frame w-8 h-8 rounded-lg border flex items-center justify-center overflow-hidden transition-colors shadow-sm border-border-default flex-shrink-0">
                    {#if item.icon_base64}
                      <img src="data:image/png;base64,{item.icon_base64}" class="app-icon w-6 h-6 object-contain" alt="" />
                    {:else}
                      <div class="w-full h-full flex items-center justify-center font-mono text-xs {getAvatarStyle(item.name)}">
                        {item.name.charAt(0).toUpperCase()}
                      </div>
                    {/if}
                  </div>
                  <div class="flex flex-col min-w-0">
                    <span class="text-text-primary group-hover:text-accent transition-colors truncate" title={item.name}>{item.name}</span>
                    {#if item.target_missing}
                      <span class="shrink-0" title={t("Target path does not exist")}>
                        <AlertTriangle class="w-3.5 h-3.5 text-danger" />
                      </span>
                    {/if}
                  </div>
                </td>
                
                <!-- Location -->
                <td class="py-4 px-3 lg:px-5 text-text-secondary max-w-[200px] truncate">
                  <span class="inline-block max-w-full truncate align-middle px-2 py-0.5 rounded text-[10px] font-mono border
                    {item.location.startsWith('HKLM') 
                      ? 'bg-warning/10 text-warning border-warning/25' 
                      : item.location.startsWith('HKCU')
                        ? 'bg-brand-purple/10 text-brand-purple border-brand-purple/20' 
                        : 'bg-info/10 text-info border-info/25'}">
                    {item.location}
                  </span>
                </td>

                <!-- Command -->
                <td class="py-4 px-3 lg:px-5 text-text-muted font-mono text-xs max-w-[250px] truncate" title={item.command}>
                  {item.command}
                </td>

                <!-- Status (Toggle) -->
                <td class="py-4 px-3 lg:px-5">
                  <div class="flex items-center">
                    <button
                      onclick={() => toggleStatus(item)}
                      title={t("Toggle startup status")}
                      aria-label={t("Toggle startup status")}
                      class="relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none
                        {item.enabled ? 'bg-accent' : 'bg-surface-bg border-border-default'}"
                    >
                      <span
                        class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out
                          {item.enabled ? 'translate-x-4' : 'translate-x-0'}"
                      ></span>
                    </button>
                    <span class="ml-2 text-xs font-semibold {item.enabled ? 'text-accent' : 'text-text-muted'}">
                      {item.enabled ? t("Enabled") : t("Disabled")}
                    </span>
                  </div>
                </td>

                <!-- Actions -->
                <td class="py-4 px-3 lg:px-5 text-right font-sans">
                  <button
                    onclick={() => deleteItem(item)}
                    title={t("Delete permanently")}
                    class="p-1.5 rounded-lg bg-danger/10 hover:bg-danger-strong text-danger hover:text-white transition-all border border-danger/20 inline-flex items-center justify-center active:scale-95 cursor-pointer"
                  >
                    <Trash2 class="w-4 h-4" />
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>
</div>
