<script lang="ts">
  import Sidebar from "$lib/components/Sidebar.svelte";
  import AppsTab from "$lib/components/AppsTab.svelte";
  import DevToolsTab from "$lib/components/DevToolsTab.svelte";
  import SnapshotsTab from "$lib/components/SnapshotsTab.svelte";
  import PathCleanerTab from "$lib/components/PathCleanerTab.svelte";
  import SettingsTab from "$lib/components/SettingsTab.svelte";

  let activeTab = $state("apps");
</script>

<div class="flex h-screen w-screen overflow-hidden bg-app-bg text-text-primary font-sans">
  <!-- Sidebar Navigation -->
  <Sidebar bind:activeTab={activeTab} />

  <!-- Main Content Viewer -->
  <main class="flex-1 ml-16 h-screen overflow-hidden flex flex-col bg-app-bg relative transition-all duration-300">
    {#if activeTab === "apps"}
      <AppsTab />
    {:else if activeTab === "devtools"}
      <DevToolsTab />
    {:else if activeTab === "snapshots"}
      <SnapshotsTab />
    {:else if activeTab === "pathcleaner"}
      <PathCleanerTab />
    {:else if activeTab === "settings"}
      <SettingsTab />
    {/if}
  </main>
</div>
