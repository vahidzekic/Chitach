<script lang="ts">
  // Under the vehicle card: fines check and vehicle history, or the Pro lock.
  import FinesPanel from "./FinesPanel.svelte";
  import HistoryPanel from "./HistoryPanel.svelte";
  import ProLock from "./ProLock.svelte";
  import { app } from "../stores/app.svelte";
</script>

{#if app.license && (!app.license.enabled || app.license.active)}
  <FinesPanel />

  <HistoryPanel />
{:else if app.license}
  <ProLock />
{/if}
