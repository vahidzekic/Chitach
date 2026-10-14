<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PinDialog from "./PinDialog.svelte";
  import { api } from "../api";
  import type { CertInfo } from "../api/types";
  import { t, translateError } from "../i18n/index.svelte";
  import { app, openPath, saveFileAs, showError } from "../stores/app.svelte";
  import { toast } from "../stores/toasts.svelte";
  import { baseName, formatCertDate } from "../util/format";

  let certs = $state<CertInfo[] | null>(null);
  let loadError = $state<string | null>(null);
  let tries = $state<number | null>(null);
  let pinOpen = $state(false);

  async function loadTries() {
    try {
      tries = await api.pinTriesLeft();
    } catch {
      tries = null;
    }
  }

  async function load() {
    certs = null;
    loadError = null;
    try {
      certs = await api.listCertificates();
    } catch (e) {
      loadError = typeof e === "string" ? e : String(e);
      certs = [];
    }
    void loadTries();
  }

  $effect(() => {
    // reload whenever a new card is read
    void app.state.readId;
    void load();
  });

  function cn(dn: string): string {
    const m = /CN=([^,]+)/.exec(dn);
    return m ? m[1] : dn;
  }

  async function save(c: CertInfo) {
    try {
      const name = `${c.label.replace(/[^\p{L}\p{N}_-]+/gu, "_").toLowerCase() || "certificate"}.cer`;
      const path = await saveFileAs(name, "cer", "DER");
      if (!path) return;
      await api.saveCertificate(c.index, path);
      toast("success", t("ui.certSaved"), {
        detail: baseName(path),
        action: { label: t("ui.open"), run: () => void openPath(path) },
      });
    } catch (e) {
      showError(e, "error.writingCert");
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>{t("nav.certificates")}</h1>
      <p>{t("certs.subtitle")}</p>
    </div>
  </div>

  <section class="card pin">
    <div class="pin-ic"><Icon name="lock" size={22} /></div>
    <div class="pin-body">
      <h2>{t("certs.pin")}</h2>
      <p>{t("certs.pinDesc")}</p>
    </div>
    <div class="pin-side">
      {#if tries !== null}
        <div class="tries" aria-label="{t('pinChange.triesLeft')}: {tries}">
          <span class="tries-label">{t("pinChange.triesLeft")}</span>
          <span class="dots">
            {#each [0, 1, 2] as i (i)}
              <span class="d" class:on={i < tries} class:low={tries <= 1}></span>
            {/each}
          </span>
        </div>
      {/if}
      <button class="btn btn-primary" type="button" onclick={() => (pinOpen = true)} disabled={tries === 0}>
        <Icon name="key" size={16} />{t("crypto.changePin")}
      </button>
    </div>
  </section>

  <h2 class="list-title">{t("certs.list")}</h2>
  {#if certs === null}
    <div class="list">
      {#each [0, 1] as i (i)}
        <div class="card cert skeleton"><div class="sk"></div></div>
      {/each}
    </div>
  {:else if certs.length === 0}
    <div class="card none">
      <Icon name="shield" size={28} />
      <p>{loadError ? translateError(loadError).title : t("crypto.noCertFound")}</p>
    </div>
  {:else}
    <div class="list">
      {#each certs as c (c.index)}
        <article class="card cert">
          <div class="cert-ic"><Icon name="shield" size={20} /></div>
          <div class="cert-body">
            <h3>{c.label}</h3>
            <div class="cn" title={c.subject}>{cn(c.subject)}</div>
            <dl>
              <div><dt>{t("crypto.issuer")}</dt><dd title={c.issuer}>{cn(c.issuer)}</dd></div>
              <div><dt>{t("certs.validTo")}</dt><dd>{formatCertDate(c.notAfter)}</dd></div>
            </dl>
          </div>
          <button class="btn" type="button" onclick={() => save(c)}>
            <Icon name="download" size={16} />{t("crypto.saveCert")}
          </button>
        </article>
      {/each}
    </div>
  {/if}
</div>

<PinDialog bind:open={pinOpen} triesLeft={tries} onchanged={loadTries} />

<style>
  .pin {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 20px;
    margin-bottom: 26px;
    background: var(--hero-grad);
  }
  .pin-ic,
  .cert-ic {
    flex: none;
    width: 46px;
    height: 46px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-contrast);
    box-shadow: var(--shadow);
  }
  .pin-body {
    flex: 1;
    min-width: 0;
  }
  .pin-body h2 {
    font-size: 16px;
  }
  .pin-body p {
    margin-top: 3px;
    color: var(--text-2);
    font-size: 13px;
  }
  .pin-side {
    display: flex;
    align-items: center;
    gap: 18px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .tries {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
  }
  .tries-label {
    font-size: 11.5px;
    color: var(--text-3);
    font-weight: 550;
  }
  .dots {
    display: flex;
    gap: 5px;
  }
  .d {
    width: 22px;
    height: 6px;
    border-radius: 3px;
    background: var(--border-strong);
  }
  .d.on {
    background: var(--success);
  }
  .d.on.low {
    background: var(--danger);
  }
  .list-title {
    font-size: 15px;
    margin-bottom: 12px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .cert {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
    animation: fade-up 0.3s var(--ease) both;
  }
  .cert-ic {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent-text);
    box-shadow: none;
  }
  .cert-body {
    flex: 1;
    min-width: 0;
  }
  h3 {
    font-size: 14.5px;
  }
  .cn {
    font-size: 13px;
    color: var(--text-2);
    margin-top: 1px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 22px;
    margin: 8px 0 0;
    font-size: 12.5px;
  }
  dt {
    display: inline;
    color: var(--text-3);
  }
  dt::after {
    content: ": ";
  }
  dd {
    display: inline;
    margin: 0;
    color: var(--text-2);
    font-weight: 550;
  }
  .none {
    padding: 32px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text-3);
  }
  .skeleton {
    height: 92px;
  }
  .sk {
    width: 100%;
    height: 100%;
    border-radius: 8px;
    background: var(--skeleton);
  }
  @media (max-width: 760px) {
    .pin {
      flex-wrap: wrap;
    }
  }
</style>
