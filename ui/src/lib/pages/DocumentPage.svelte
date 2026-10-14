<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import IdView from "./document/IdView.svelte";
  import MedicalView from "./document/MedicalView.svelte";
  import VehicleView from "./document/VehicleView.svelte";
  import StatusView from "./document/StatusView.svelte";
  import { api } from "../api";
  import type { ExportFormat } from "../api/types";
  import { t } from "../i18n/index.svelte";
  import { app, exportDocument, showError } from "../stores/app.svelte";
  import { toast } from "../stores/toasts.svelte";

  const doc = $derived(app.state.status === "ready" ? app.state.document : null);
  let busy = $state<null | ExportFormat | "rfzo" | "reload">(null);

  async function save(format: ExportFormat) {
    if (busy) return;
    busy = format;
    try {
      await exportDocument(format);
    } finally {
      busy = null;
    }
  }

  async function reload() {
    if (busy) return;
    busy = "reload";
    try {
      await api.reloadCard();
    } catch (e) {
      showError(e);
    } finally {
      busy = null;
    }
  }

  async function updateRfzo() {
    if (busy) return;
    busy = "rfzo";
    try {
      const d = await api.updateRfzo();
      app.state.document = d;
      toast("success", t("ui.updateSuccessful"));
    } catch (e) {
      showError(e, "error.dataUpdate");
    } finally {
      busy = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!doc || app.page !== "document") return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save("pdf");
    } else if ((mod && e.key.toLowerCase() === "r") || e.key === "F5") {
      e.preventDefault();
      void reload();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if doc}
  <div class="doc">
    <div class="page content">
      {#key app.state.readId}
        {#if doc.kind === "id"}
          <IdView doc={doc.data} />
        {:else if doc.kind === "medical"}
          <MedicalView doc={doc.data} />
        {:else}
          <VehicleView doc={doc.data} />
        {/if}
      {/key}
    </div>

    <div class="actionbar" role="toolbar" aria-label={t("ui.documentActions")}>
      <div class="bar-inner">
        <div class="left">
          <button class="btn btn-ghost" type="button" onclick={reload} disabled={!!busy} title="{t('ui.reload')} (F5)" aria-label={t("ui.reload")}>
            <Icon name="refresh" size={16} class={busy === "reload" ? "spin" : ""} />
            <span class="collapse">{t("ui.reload")}</span>
          </button>
          {#if doc.kind === "medical"}
            <button class="btn btn-ghost" type="button" onclick={updateRfzo} disabled={!!busy} title={t("ui.updateRfzo")} aria-label={t("ui.updateRfzo")}>
              <Icon name={busy === "rfzo" ? "refresh" : "cloud"} size={16} class={busy === "rfzo" ? "spin" : ""} />
              <span class="collapse-sm">{busy === "rfzo" ? t("ui.updating") : t("ui.updateRfzo")}</span>
            </button>
          {/if}
        </div>
        <div class="right">
          <span class="save-label">{t("ui.saveAs")}</span>
          <div class="group">
            <button class="btn" type="button" onclick={() => save("json")} disabled={!!busy} title={t("ui.saveJson")}>
              <Icon name="braces" size={16} /><span>JSON</span>
            </button>
            <button class="btn" type="button" onclick={() => save("xlsx")} disabled={!!busy} title={t("ui.saveXlsx")}>
              <Icon name="table" size={16} /><span>Excel</span>
            </button>
          </div>
          <button
            class="btn btn-primary"
            type="button"
            onclick={() => save("pdf")}
            disabled={!!busy}
            title={`${t("ui.savePdf")} (Ctrl+S)`}
          >
            <Icon name={busy === "pdf" ? "refresh" : "download"} size={16} class={busy === "pdf" ? "spin" : ""} />
            <span>{t("ui.savePdf")}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
{:else}
  <StatusView />
{/if}

<style>
  .doc {
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }
  .content {
    flex: 1;
    width: 100%;
    padding-bottom: 24px;
  }
  .actionbar {
    position: sticky;
    bottom: 0;
    z-index: 5;
    border-top: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    backdrop-filter: blur(12px) saturate(1.4);
  }
  .bar-inner {
    max-width: 1080px;
    margin: 0 auto;
    padding: 12px 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .save-label {
    font-size: 12.5px;
    color: var(--text-3);
    font-weight: 550;
    margin-right: 2px;
  }
  .group {
    display: inline-flex;
  }
  .group .btn:first-child {
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
  }
  .group .btn:last-child {
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    margin-left: -1px;
  }
  .bar-inner {
    flex-wrap: nowrap;
  }
  @media (max-width: 1000px) {
    .doc:has(.collapse-sm) .collapse {
      display: none;
    }
  }
  @media (max-width: 900px) {
    .save-label {
      display: none;
    }
    .bar-inner {
      padding: 10px 20px;
    }
  }
</style>
