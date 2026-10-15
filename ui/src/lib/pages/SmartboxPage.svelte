<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { api } from "../api";
  import type { SmartboxStatus, Unlisten } from "../api/types";
  import { t, translateError } from "../i18n/index.svelte";
  import { navigate, showError } from "../stores/app.svelte";
  import { copyText } from "../util/format";
  import { toast } from "../stores/toasts.svelte";

  let status = $state<SmartboxStatus | null>(null);
  let unlisten: Unlisten | undefined;

  onMount(async () => {
    try {
      status = await api.smartboxStatus();
    } catch (e) {
      showError(e);
    }
    unlisten = await api.onSmartboxStatus((s) => (status = s));
  });
  onDestroy(() => unlisten?.());

  const err = $derived(status?.error ? translateError(status.error) : null);
  const port = $derived(status?.address?.split(":").pop() ?? "");

  async function copyAddr() {
    if (status?.address && (await copyText(status.address))) toast("success", t("ui.contentCopied"));
  }
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>{t("smartbox.title")}</h1>
      <p>{t("smartbox.subtitle")}</p>
    </div>
  </div>

  {#if !status}
    <div class="card hero skeleton"></div>
  {:else}
    <section class="card hero" class:running={status.running} class:failed={!status.running}>
      <div class="pulse" aria-hidden="true">
        <span></span>
        <Icon name="server" size={26} />
      </div>
      <div class="hero-body">
        <h2>{status.running ? t("smartbox.running") : t("smartbox.stopped")}</h2>
        {#if status.running && status.address}
          <div class="addr">
            <span class="faint">{t("smartbox.address")}</span>
            <code class="mono">{status.address}</code>
            <button class="btn btn-sm btn-ghost btn-icon" type="button" onclick={copyAddr} aria-label={t("ui.copy")}>
              <Icon name="copy" size={14} />
            </button>
            {#if port}<span class="badge">{t("smartbox.port")} {port}</span>{/if}
          </div>
        {/if}
        {#if err}
          <div class="err" role="alert">
            <Icon name="alertCircle" size={16} />
            <span><strong>{t("smartbox.error")}:</strong> {err.title}{#if err.detail} — {err.detail}{/if}</span>
          </div>
        {/if}
      </div>
    </section>

    <div class="grid">
      <section class="card box">
        <h3><Icon name="chip" size={16} />{t("smartbox.vendors")}</h3>
        {#if status.vendors.length === 0}
          <p class="muted">{t("smartbox.noVendors")}</p>
          <button class="btn btn-sm" type="button" onclick={() => navigate("settings")}>
            <Icon name="sliders" size={14} />{t("ui.goToSettings")}
          </button>
        {:else}
          <ul class="vendors">
            {#each status.vendors as v (v)}
              <li><span class="ok"><Icon name="check" size={13} stroke={2.6} /></span>{t(`vendor.${v}`)}</li>
            {/each}
          </ul>
        {/if}
      </section>
      <section class="card box">
        <h3><Icon name="info" size={16} />ePorezi</h3>
        <p class="muted">{t("smartbox.howTo")}</p>
      </section>
    </div>
  {/if}
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 24px;
    margin-bottom: 16px;
    background: var(--hero-grad);
  }
  .skeleton {
    height: 120px;
  }
  .pulse {
    position: relative;
    flex: none;
    width: 64px;
    height: 64px;
    border-radius: 20px;
    display: grid;
    place-items: center;
    color: #fff;
    background: var(--text-3);
  }
  .running .pulse {
    background: var(--success);
  }
  .failed .pulse {
    background: var(--danger);
  }
  .running .pulse span {
    position: absolute;
    inset: 0;
    border-radius: 20px;
    border: 2px solid var(--success);
    animation: ring 2s ease-out infinite;
  }
  @keyframes ring {
    from {
      transform: scale(1);
      opacity: 0.8;
    }
    to {
      transform: scale(1.45);
      opacity: 0;
    }
  }
  .hero-body {
    flex: 1;
    min-width: 0;
  }
  .hero-body h2 {
    font-size: 19px;
  }
  .addr {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .addr code {
    padding: 3px 8px;
    border-radius: 6px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .err {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 10px;
    color: var(--danger-text);
    font-size: 13px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
  }
  .box {
    padding: 18px;
  }
  .box h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14.5px;
    margin-bottom: 10px;
  }
  .box .btn {
    margin-top: 10px;
  }
  .vendors {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .vendors li {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 550;
  }
  .ok {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--success-soft);
    color: var(--success);
  }
</style>
