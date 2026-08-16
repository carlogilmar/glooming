<script lang="ts">
  import "../app.css";
  import { theme } from "$lib/stores/theme.svelte";

  let { children } = $props();

  // Apply the stored theme before the first paint of the app shell.
  theme.init();
</script>

{@render children()}
