<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import Toast from "$lib/components/Toast.svelte";
  import { theme } from "$lib/theme.svelte";
  import { i18n } from "$lib/i18n.svelte";
  let { children } = $props();

  onMount(() => {
    theme.init();
    i18n.init();
    return () => theme.destroy();
  });
</script>

{@render children()}
<Toast />

