<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import Toast from "$lib/components/Toast.svelte";
  import { theme } from "$lib/theme.svelte";
  let { children } = $props();

  onMount(() => {
    theme.init();
    return () => theme.destroy();
  });
</script>

{@render children()}
<Toast />

