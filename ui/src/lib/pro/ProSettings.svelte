<script lang="ts">
  // Pro sections of the settings page: Chitach Pro licence, fines check, vehicle history
  // and PKCS#11 modules. Styles come from ../pages/settings.css.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import Switch from "../components/Switch.svelte";
  import { api } from "./api";
  import { CARD_VENDORS, type FinesProvider, type HistoryProvider, type Settings } from "./types";
  import { hasKey, t } from "../i18n/index.svelte";
  import { app, showError } from "../stores/app.svelte";
  import { toast } from "../stores/toasts.svelte";
  import { activateLicense, deactivateLicense } from "./license";

  let { draft = $bindable(), isMobile }: { draft: Settings; isMobile: boolean } = $props();

  let proKey = $state("");
  let proBusy = $state(false);

  async function activatePro() {
    proBusy = true;
    if (await activateLicense(proKey)) proKey = "";
    proBusy = false;
  }

  async function deactivatePro() {
    proBusy = true;
    await deactivateLicense();
    proBusy = false;
  }

  async function pickModule(v: (typeof CARD_VENDORS)[number]) {
    const ext = app.info?.os === "windows" ? ["dll"] : app.info?.os === "macos" ? ["dylib", "so"] : ["so"];
    const p = await api.openFileDialog({ filters: [{ name: "PKCS#11", extensions: ext }] });
    if (p) draft.pkcs11Paths[v] = p;
  }

  // Fines check: providers with an automatic check.
  let autoProviders = $state<FinesProvider[]>([]);
  onMount(async () => {
    try {
      autoProviders = (await api.finesProviders()).providers.filter((p) => p.auto);
    } catch {
      autoProviders = [];
    }
  });
  function providerName(p: FinesProvider): string {
    const key = `fines.provider.${p.id}.name`;
    return hasKey(key) ? t(key) : p.name;
  }
  function providerEnabled(id: string): boolean {
    return !draft.fines.enabledProviders || draft.fines.enabledProviders.includes(id);
  }
  function setProviderEnabled(id: string, on: boolean) {
    const ids = draft.fines.enabledProviders ?? autoProviders.map((p) => p.id);
    draft.fines.enabledProviders = on ? [...new Set([...ids, id])] : ids.filter((x) => x !== id);
  }

  // Vehicle history: API providers with the user's own key (stored by the backend, not in
  // the settings, so saved immediately).
  let historyApis = $state<HistoryProvider[]>([]);
  let keyInput = $state<Record<string, { apiKey: string; secret: string }>>({});
  let keyBusy = $state<Record<string, boolean>>({});
  async function loadHistoryApis() {
    try {
      historyApis = (await api.historyInfo()).providers.filter((p) => p.kind === "api");
      for (const p of historyApis) keyInput[p.id] ??= { apiKey: "", secret: "" };
    } catch {
      historyApis = [];
    }
  }
  onMount(loadHistoryApis);
  function historyName(p: HistoryProvider): string {
    const key = `history.provider.${p.id}.name`;
    return hasKey(key) ? t(key) : p.name;
  }
  async function saveKey(p: HistoryProvider) {
    keyBusy[p.id] = true;
    try {
      await api.historySetApiKey(p.id, keyInput[p.id].apiKey, keyInput[p.id].secret);
      keyInput[p.id] = { apiKey: "", secret: "" };
      toast("success", t("settings.historyKeySaved", historyName(p)));
      await loadHistoryApis();
    } catch (e) {
      showError(e, "error.settings");
    } finally {
      keyBusy[p.id] = false;
    }
  }
  async function removeKey(p: HistoryProvider) {
    keyBusy[p.id] = true;
    try {
      await api.historyRemoveApiKey(p.id);
      toast("success", t("settings.historyKeyRemoved", historyName(p)));
      await loadHistoryApis();
    } catch (e) {
      showError(e, "error.settings");
    } finally {
      keyBusy[p.id] = false;
    }
  }
</script>

{#if app.license?.enabled}
  <section class="card group">
    <h2><Icon name="sparkle" size={16} />{t("pro.title")}</h2>
    <p class="group-desc">{t("pro.settingsDesc")}</p>
    <div class="row">
      <div class="desc">
        <label for="set-pro-key">{t("pro.key")}</label>
        <small>
          {#if app.license?.active}
            {t("pro.activeDesc", app.license.email ?? app.license.keyHint ?? "")}
          {:else if app.license?.stored}
            {t("pro.expired")}
          {:else}
            {t("pro.inactiveDesc")}
          {/if}
        </small>
      </div>
      <div class="ctl">
        {#if app.license?.stored}
          <div class="path-row">
            {#if app.license.active}
              <span class="badge badge-success"><Icon name="check" size={13} stroke={2.2} />{t("pro.active")} {app.license.keyHint}</span>
            {:else}
              <span class="badge badge-warning"><Icon name="alert" size={13} stroke={2.2} />{t("pro.notValidated")}</span>
            {/if}
            <button class="btn btn-sm btn-danger" type="button" onclick={deactivatePro} disabled={proBusy}>
              {t("pro.deactivate")}
            </button>
          </div>
        {:else}
          <div class="path-row">
            <input
              id="set-pro-key"
              class="input mono"
              type="text"
              autocomplete="off"
              spellcheck="false"
              placeholder={t("pro.keyPlaceholder")}
              bind:value={proKey}
            />
            <button class="btn" type="button" onclick={activatePro} disabled={proBusy || !proKey.trim()}>
              <Icon name="key" size={16} />{t("pro.activate")}
            </button>
            {#if app.license?.buyUrl}
              <button class="btn btn-primary" type="button" onclick={() => void api.openUrl(app.license!.buyUrl!)}>
                <Icon name="sparkle" size={16} />{t("pro.buy")}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </section>
{/if}

<section class="card group">
  <h2><Icon name="receipt" size={16} />{t("settings.fines")}</h2>
  <p class="group-desc">{t("settings.finesDesc")}</p>
  <div class="row">
    <div class="desc">
      <label for="set-fines-auto">{t("settings.finesAutoCheck")}</label>
      <small>{t("settings.finesAutoCheckDesc")}</small>
    </div>
    <div class="ctl end">
      <Switch id="set-fines-auto" bind:checked={draft.fines.autoCheck} label={t("settings.finesAutoCheck")} />
    </div>
  </div>
  <div class="row">
    <div class="desc">
      <span>{t("settings.finesConsent")}</span>
      <small>{draft.fines.consentGiven ? t("settings.finesConsentGivenDesc") : t("settings.finesConsentNotGivenDesc")}</small>
    </div>
    <div class="ctl end consent">
      {#if draft.fines.consentGiven}
        <span class="badge badge-success"><Icon name="check" size={13} stroke={2.2} />{t("settings.finesConsentGiven")}</span>
        <button class="btn btn-sm btn-danger" type="button" onclick={() => (draft.fines.consentGiven = false)}>
          {t("settings.finesRevoke")}
        </button>
      {:else}
        <span class="badge">{t("settings.finesConsentNotGiven")}</span>
      {/if}
    </div>
  </div>
  <div class="row">
    <div class="desc">
      <label for="set-fines-remote">{t("settings.finesRemote")}</label>
      <small>{t("settings.finesRemoteDesc")}</small>
    </div>
    <div class="ctl end">
      <Switch id="set-fines-remote" bind:checked={draft.fines.remoteDefinitions} label={t("settings.finesRemote")} />
    </div>
  </div>
  {#each autoProviders as p (p.id)}
    <div class="row">
      <div class="desc">
        <label for="set-fines-{p.id}">{providerName(p)}</label>
        <small>{t("settings.finesProviderDesc", p.checkDomain ?? "")}</small>
      </div>
      <div class="ctl end">
        <Switch
          id="set-fines-{p.id}"
          bind:checked={() => providerEnabled(p.id), (v) => setProviderEnabled(p.id, v)}
          label={providerName(p)}
        />
      </div>
    </div>
  {/each}
</section>

<section class="card group">
  <h2><Icon name="history" size={16} />{t("settings.history")}</h2>
  <p class="group-desc">{t("settings.historyDesc")}</p>
  <div class="row">
    <div class="desc">
      <span>{t("settings.historyConsent")}</span>
      <small>{draft.history.consentGiven ? t("settings.historyConsentGivenDesc") : t("settings.historyConsentNotGivenDesc")}</small>
    </div>
    <div class="ctl end consent">
      {#if draft.history.consentGiven}
        <span class="badge badge-success"><Icon name="check" size={13} stroke={2.2} />{t("settings.finesConsentGiven")}</span>
        <button class="btn btn-sm btn-danger" type="button" onclick={() => (draft.history.consentGiven = false)}>
          {t("settings.finesRevoke")}
        </button>
      {:else}
        <span class="badge">{t("settings.finesConsentNotGiven")}</span>
      {/if}
    </div>
  </div>
  {#each historyApis as p (p.id)}
    <div class="row">
      <div class="desc">
        <label for="set-hkey-{p.id}">{historyName(p)}</label>
        <small>{t("settings.historyKeyDesc", p.apiDomain ?? "")}</small>
      </div>
      <div class="ctl">
        {#if p.configured}
          <div class="path-row">
            <span class="badge badge-success"><Icon name="key" size={13} stroke={2.2} />{t("settings.historyKeyStored")}</span>
            <button class="btn btn-sm btn-danger" type="button" onclick={() => removeKey(p)} disabled={keyBusy[p.id]}>
              {t("settings.historyKeyRemove")}
            </button>
          </div>
        {:else if keyInput[p.id]}
          <div class="path-row">
            <input
              id="set-hkey-{p.id}"
              class="input mono"
              type="password"
              autocomplete="off"
              placeholder={t("settings.historyApiKey")}
              bind:value={keyInput[p.id].apiKey}
            />
            <input
              class="input mono"
              type="password"
              autocomplete="off"
              placeholder={t("settings.historySecret")}
              aria-label={t("settings.historySecret")}
              bind:value={keyInput[p.id].secret}
            />
            <button
              class="btn"
              type="button"
              onclick={() => saveKey(p)}
              disabled={keyBusy[p.id] || !keyInput[p.id].apiKey.trim()}
            >
              <Icon name="key" size={16} />{t("settings.historyKeySave")}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/each}
</section>

{#if !isMobile}
<section class="card group">
  <h2><Icon name="chip" size={16} />{t("settings.pkcs11")}</h2>
  <p class="group-desc">{t("settings.pkcs11Desc")}</p>
  {#each CARD_VENDORS as v (v)}
    <div class="row">
      <div class="desc">
        <label for="pk-{v}">{t(`vendor.${v}`)}</label>
      </div>
      <div class="ctl">
        <div class="path-row">
          <input
            id="pk-{v}"
            class="input mono"
            placeholder={t("settings.default")}
            bind:value={draft.pkcs11Paths[v]}
            spellcheck="false"
          />
          <button
            class="btn btn-icon"
            type="button"
            onclick={() => pickModule(v)}
            aria-label="{t('ui.browse')} {t(`vendor.${v}`)}"
            title={t("ui.browse")}
          >
            <Icon name="folder" size={16} />
          </button>
        </div>
      </div>
    </div>
  {/each}
</section>
{/if}
