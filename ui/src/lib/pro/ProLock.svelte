<script lang="ts">
  // Shown in place of a Chitach Pro feature (fines and history panels, the certificates,
  // XML signing and Smartbox pages) while Pro is not active: what Pro unlocks, a buy
  // button and the key field.
  import Icon, { type IconName } from "../components/Icon.svelte";
  import { api } from "./api";
  import type { ProFeature } from "./types";
  import { t } from "../i18n/index.svelte";
  import { app } from "../stores/app.svelte";
  import { activateLicense } from "./license";

  // One line per thing Pro unlocks; PIN change and certificates share a line.
  const FEATURES: { key: string; icon: IconName; covers: ProFeature[] }[] = [
    { key: "pro.featureFines", icon: "receipt", covers: ["fines"] },
    { key: "pro.featureHistory", icon: "history", covers: ["history"] },
    { key: "pro.featureExport", icon: "table", covers: ["dataExport", "autoSave"] },
    { key: "pro.featureCertificates", icon: "shield", covers: ["pin", "certificates"] },
    { key: "pro.featureXmlSign", icon: "pen", covers: ["xmlSign"] },
    { key: "pro.featureSmartbox", icon: "server", covers: ["smartbox"] },
    { key: "pro.featureBackground", icon: "monitor", covers: ["background"] },
  ];
  const features = $derived(
    FEATURES.filter((f) => f.covers.some((c) => app.license?.features.includes(c) ?? true)),
  );

  let key = $state("");
  let busy = $state(false);
  let showKey = $state(false);

  async function activate() {
    busy = true;
    await activateLicense(key);
    busy = false;
  }
</script>

<section class="pro card" aria-labelledby="pro-title">
  <header>
    <span class="ic"><Icon name="lock" size={16} /></span>
    <div class="htext">
      <h2 id="pro-title">{t("pro.lockTitle")}</h2>
      <p>{t("pro.lockSubtitle")}</p>
    </div>
    <div class="hactions">
      {#if app.license?.buyUrl}
        <button class="btn btn-sm btn-primary" type="button" onclick={() => void api.openUrl(app.license!.buyUrl!)}>
          <Icon name="sparkle" size={14} />{t("pro.buy")}
        </button>
      {/if}
      {#if !showKey}
        <button class="btn btn-sm" type="button" onclick={() => (showKey = true)}>
          <Icon name="key" size={14} />{t("pro.haveKey")}
        </button>
      {/if}
    </div>
  </header>
  <ul>
    {#each features as f (f.key)}
      <li><Icon name={f.icon} size={14} />{t(f.key)}</li>
    {/each}
  </ul>
  {#if app.license?.stored && !app.license.active}
    <p class="warn">{t("pro.expired")}</p>
  {/if}
  {#if showKey}
    <form
      class="key"
      onsubmit={(e) => {
        e.preventDefault();
        void activate();
      }}
    >
      <input
        class="input mono"
        type="text"
        autocomplete="off"
        spellcheck="false"
        placeholder={t("pro.keyPlaceholder")}
        aria-label={t("pro.key")}
        bind:value={key}
      />
      <button class="btn btn-primary" type="submit" disabled={busy || !key.trim()}>
        <Icon name={busy ? "refresh" : "key"} size={16} class={busy ? "spin" : ""} />{t("pro.activate")}
      </button>
    </form>
  {/if}
  <p class="note">{t("pro.note")}</p>
</section>

<style>
  .pro {
    padding: 16px 18px 14px;
    margin-bottom: 16px;
    animation: fade-up 0.35s var(--ease) both;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .htext {
    flex: 1;
    min-width: 200px;
  }
  h2 {
    font-size: 14.5px;
    font-weight: 650;
  }
  .htext p,
  .note {
    font-size: 12.5px;
    color: var(--text-3);
  }
  .hactions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  ul {
    list-style: none;
    display: grid;
    gap: 6px;
    margin: 4px 0 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  li :global(svg) {
    color: var(--accent-text);
    flex: none;
  }
  .warn {
    font-size: 12.5px;
    color: var(--warning-text);
    margin-bottom: 8px;
  }
  .key {
    display: flex;
    gap: 8px;
    margin-bottom: 10px;
  }
  .key .input {
    flex: 1;
    min-width: 0;
  }
</style>
