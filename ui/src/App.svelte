<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import DocumentPage from "./lib/pages/DocumentPage.svelte";
  import CertificatesPage from "./lib/pages/CertificatesPage.svelte";
  import XmlSignPage from "./lib/pages/XmlSignPage.svelte";
  import SmartboxPage from "./lib/pages/SmartboxPage.svelte";
  import SettingsPage from "./lib/pages/SettingsPage.svelte";
  import AboutPage from "./lib/pages/AboutPage.svelte";
  import { app, init } from "./lib/stores/app.svelte";

  let main: HTMLElement | undefined = $state();

  onMount(() => {
    void init();
  });

  // Scroll to top and move focus target on page change.
  $effect(() => {
    void app.page;
    main?.scrollTo({ top: 0 });
  });
</script>

<div class="shell">
  <Sidebar />
  <main bind:this={main} tabindex="-1">
    {#if app.loaded && app.settings}
      {#if app.page === "document"}
        <DocumentPage />
      {:else if app.page === "certificates"}
        <CertificatesPage />
      {:else if app.page === "xmlSign"}
        <XmlSignPage />
      {:else if app.page === "smartbox"}
        <SmartboxPage />
      {:else if app.page === "settings"}
        <SettingsPage />
      {:else}
        <AboutPage />
      {/if}
    {/if}
  </main>
</div>
<Toasts />

<style>
  .shell {
    display: flex;
    height: 100vh;
    min-width: 0;
  }
  main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
    outline: none;
  }
</style>
