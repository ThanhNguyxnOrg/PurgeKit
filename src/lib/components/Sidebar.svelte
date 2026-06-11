<script lang="ts">
  import { Monitor, Terminal, Layers, FileCode, Settings, Zap } from "@lucide/svelte";

  let { activeTab = $bindable() } = $props();

  const mainMenuItems = [
    { id: "apps", label: "Apps Manager", icon: Monitor },
    { id: "devtools", label: "Dev Tools", icon: Terminal },
    { id: "startup", label: "Startup Manager", icon: Zap },
    { id: "snapshots", label: "System Snapshots", icon: Layers },
    { id: "pathcleaner", label: "PATH Cleaner", icon: FileCode },
  ];
</script>

<aside class="fixed top-0 left-0 h-screen z-30 bg-sidebar-bg border-r border-border-default flex flex-col transition-all duration-300 ease-in-out w-16 hover:w-64 group/sidebar shadow-xl">
  <!-- Brand Header -->
  <div class="h-16 border-b border-border-default flex items-center px-3 gap-3 overflow-hidden">
    <div class="w-10 h-10 rounded-lg bg-surface-bg border border-border-default flex items-center justify-center flex-shrink-0">
      <img src="/logo.png" alt="PurgeKit Logo" class="w-6 h-6 object-contain" />
    </div>
    <div class="flex flex-col opacity-0 group-hover/sidebar:opacity-100 transition-opacity duration-200 delay-75 whitespace-nowrap overflow-hidden">
      <h1 class="text-sm font-bold text-text-primary tracking-wide leading-none mb-1">
        PurgeKit
      </h1>
      <span class="text-[9px] text-text-muted font-mono uppercase tracking-wider">Ultimate Uninstaller</span>
    </div>
  </div>

  <!-- Navigation Menu -->
  <nav class="flex-1 py-4 px-2 space-y-1 overflow-y-auto overflow-x-hidden">
    {#each mainMenuItems as item}
      {@const Icon = item.icon}
      <button
        onclick={() => activeTab = item.id}
        class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all duration-150 group/btn relative overflow-hidden
          {activeTab === item.id
            ? 'text-accent bg-accent-subtle border-accent/20'
            : 'text-text-secondary hover:text-text-primary hover:bg-elevated-bg border-transparent border'}"
      >
        <!-- Highlight indicator left -->
        {#if activeTab === item.id}
          <div class="absolute left-0 top-2 bottom-2 w-1 bg-accent rounded-r-md"></div>
        {/if}

        <Icon class="w-5 h-5 flex-shrink-0 transition-transform duration-200 group-hover/btn:scale-105
          {activeTab === item.id ? 'text-accent' : 'text-text-secondary group-hover/btn:text-text-primary'}" />
        
        <span class="opacity-0 group-hover/sidebar:opacity-100 transition-opacity duration-200 delay-75 whitespace-nowrap font-sans font-normal">
          {item.label}
        </span>
      </button>
    {/each}
  </nav>

  <!-- Bottom Section (Settings) -->
  <div class="p-2 border-t border-border-default">
    <button
      onclick={() => activeTab = 'settings'}
      class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all duration-150 group/btn relative overflow-hidden
        {activeTab === 'settings'
          ? 'text-accent bg-accent-subtle border-accent/20'
          : 'text-text-secondary hover:text-text-primary hover:bg-elevated-bg border-transparent border'}"
    >
      {#if activeTab === 'settings'}
        <div class="absolute left-0 top-2 bottom-2 w-1 bg-accent rounded-r-md"></div>
      {/if}

      <Settings class="w-5 h-5 flex-shrink-0 transition-transform duration-200 group-hover/btn:rotate-45
        {activeTab === 'settings' ? 'text-accent' : 'text-text-secondary group-hover/btn:text-text-primary'}" />
      
      <span class="opacity-0 group-hover/sidebar:opacity-100 transition-opacity duration-200 delay-75 whitespace-nowrap font-sans font-normal">
        Settings
      </span>
    </button>
  </div>
</aside>
