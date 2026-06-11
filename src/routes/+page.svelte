<script lang="ts">
  import Sidebar from "$lib/components/Sidebar.svelte";
  import AppsTab from "$lib/components/AppsTab.svelte";
  import DevToolsTab from "$lib/components/DevToolsTab.svelte";
  import StartupTab from "$lib/components/StartupTab.svelte";
  import SnapshotsTab from "$lib/components/SnapshotsTab.svelte";
  import PathCleanerTab from "$lib/components/PathCleanerTab.svelte";
  import SettingsTab from "$lib/components/SettingsTab.svelte";

  let activeTab = $state("apps");
</script>

<div class="flex h-screen w-screen overflow-hidden bg-app-bg text-text-primary font-sans pl-16">
  <!-- Sidebar Navigation -->
  <Sidebar bind:activeTab={activeTab} />

  <!-- Main Content Viewer -->
  <main class="flex-1 h-screen overflow-hidden flex flex-col bg-app-bg relative transition-all duration-300">
    <div class="flex-1 flex flex-col h-full {activeTab === 'apps' ? '' : 'hidden'}">
      <AppsTab />
    </div>
    <div class="flex-1 flex flex-col h-full {activeTab === 'devtools' ? '' : 'hidden'}">
      <DevToolsTab />
    </div>
    <div class="flex-1 flex flex-col h-full {activeTab === 'startup' ? '' : 'hidden'}">
      <StartupTab />
    </div>
    <div class="flex-1 flex flex-col h-full {activeTab === 'snapshots' ? '' : 'hidden'}">
      <SnapshotsTab />
    </div>
    <div class="flex-1 flex flex-col h-full {activeTab === 'pathcleaner' ? '' : 'hidden'}">
      <PathCleanerTab />
    </div>
    <div class="flex-1 flex flex-col h-full {activeTab === 'settings' ? '' : 'hidden'}">
      <SettingsTab />
    </div>
  </main>
</div>
