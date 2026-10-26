<script lang="ts">
  import Sidebar from "$lib/components/Sidebar.svelte";
  import AppsTab from "$lib/components/AppsTab.svelte";
  import DevToolsTab from "$lib/components/DevToolsTab.svelte";
  import ProjectSweeperTab from "$lib/components/ProjectSweeperTab.svelte";
  import DiskSpaceTab from "$lib/components/DiskSpaceTab.svelte";
  import WslShrinkerTab from "$lib/components/WslShrinkerTab.svelte";
  import StartupTab from "$lib/components/StartupTab.svelte";
  import SnapshotsTab from "$lib/components/SnapshotsTab.svelte";
  import PathCleanerTab from "$lib/components/PathCleanerTab.svelte";
  import SettingsTab from "$lib/components/SettingsTab.svelte";

  let activeTab = $state("apps");
  let visitedTabs = $state<Record<string, boolean>>({ apps: true });

  $effect(() => {
    if (!visitedTabs[activeTab]) {
      visitedTabs = { ...visitedTabs, [activeTab]: true };
    }
  });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-app-bg text-text-primary font-sans pl-16">
  <!-- Sidebar Navigation -->
  <Sidebar bind:activeTab={activeTab} />

  <!-- Main Content Viewer -->
  <main class="flex-1 h-screen overflow-hidden flex flex-col bg-app-bg relative transition-all duration-300">
    {#if visitedTabs.apps}
      <div class="flex-1 flex flex-col h-full {activeTab === 'apps' ? '' : 'hidden'}">
        <AppsTab />
      </div>
    {/if}
    {#if visitedTabs.devtools}
      <div class="flex-1 flex flex-col h-full {activeTab === 'devtools' ? '' : 'hidden'}">
        <DevToolsTab />
      </div>
    {/if}
    {#if visitedTabs.projectsweeper}
      <div class="flex-1 flex flex-col h-full {activeTab === 'projectsweeper' ? '' : 'hidden'}">
        <ProjectSweeperTab />
      </div>
    {/if}
    {#if visitedTabs.diskspace}
      <div class="flex-1 flex flex-col h-full {activeTab === 'diskspace' ? '' : 'hidden'}">
        <DiskSpaceTab />
      </div>
    {/if}
    {#if visitedTabs.wslshrinker}
      <div class="flex-1 flex flex-col h-full {activeTab === 'wslshrinker' ? '' : 'hidden'}">
        <WslShrinkerTab />
      </div>
    {/if}
    {#if visitedTabs.startup}
      <div class="flex-1 flex flex-col h-full {activeTab === 'startup' ? '' : 'hidden'}">
        <StartupTab />
      </div>
    {/if}
    {#if visitedTabs.snapshots}
      <div class="flex-1 flex flex-col h-full {activeTab === 'snapshots' ? '' : 'hidden'}">
        <SnapshotsTab />
      </div>
    {/if}
    {#if visitedTabs.pathcleaner}
      <div class="flex-1 flex flex-col h-full {activeTab === 'pathcleaner' ? '' : 'hidden'}">
        <PathCleanerTab />
      </div>
    {/if}
    {#if visitedTabs.settings}
      <div class="flex-1 flex flex-col h-full {activeTab === 'settings' ? '' : 'hidden'}">
        <SettingsTab />
      </div>
    {/if}
  </main>
</div>
