<script lang="ts">
  // Shown instead of the fines check and the vehicle history panels while Chitach Pro is
  // not activated: what Pro unlocks, a buy button and the key field.
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../api";
  import { t } from "../../i18n/index.svelte";
  import { activateLicense, app } from "../../stores/app.svelte";

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
    <li><Icon name="receipt" size={14} />{t("pro.featureFines")}</li>
    <li><Icon name="history" size={14} />{t("pro.featureHistory")}</li>
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
