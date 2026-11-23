<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import DocumentPage from "./lib/pages/DocumentPage.svelte";
  import SettingsPage from "./lib/pages/SettingsPage.svelte";
  import AboutPage from "./lib/pages/AboutPage.svelte";
  import { pro } from "$pro";
  import { app, init, proAvailable, proUnlocked } from "./lib/stores/app.svelte";
  import { t } from "./lib/i18n/index.svelte";

  let main: HTMLElement | undefined = $state();

  onMount(() => {
    void init();
  });

  const proPage = $derived(
    app.page === "certificates" || app.page === "xmlSign" || app.page === "smartbox" ? app.page : null,
  );
  const ProPage = $derived(proPage && proAvailable() && pro.pages ? pro.pages[proPage] : null);

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
      {:else if proPage && ProPage && !proUnlocked() && pro.ProLock}
        <div class="page">
          <div class="page-header"><div><h1>{t(`nav.${proPage}`)}</h1></div></div>
          <pro.ProLock />
        </div>
      {:else if ProPage}
        <ProPage />
      {:else if proPage}
        <DocumentPage />
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
  @media (max-width: 720px) {
    .shell {
      flex-direction: column;
      height: 100dvh;
    }
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
