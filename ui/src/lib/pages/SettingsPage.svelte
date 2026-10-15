<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Switch from "../components/Switch.svelte";
  import { api } from "../api";
  import {
    CARD_VENDORS,
    LANGUAGES,
    type AutoSave,
    type FinesProvider,
    type Mode,
    type Settings,
    type Theme,
  } from "../api/types";
  import { hasKey, languageNames, t } from "../i18n/index.svelte";
  import { app, previewSettings, saveSettings } from "../stores/app.svelte";
  import { toast } from "../stores/toasts.svelte";

  const clone = (s: Settings): Settings => structuredClone($state.snapshot(s) as Settings);

  // app.settings is loaded before any page is shown
  let draft = $state<Settings>(clone(app.settings!));
  let saving = $state(false);

  const dirty = $derived(JSON.stringify(draft) !== JSON.stringify(app.settings));

  // live preview of theme & language
  $effect(() => {
    previewSettings(draft);
  });

  onDestroy(() => {
    if (app.settings) previewSettings(app.settings);
  });

  function discard() {
    if (app.settings) draft = clone(app.settings);
  }

  async function save() {
    saving = true;
    const modeChanged = draft.mode !== app.mode;
    const ok = await saveSettings(draft);
    saving = false;
    if (ok) {
      toast("success", t("preference.saved"), { detail: modeChanged ? t("preference.startAgain") : undefined });
    }
  }

  async function pickDir() {
    const d = await api.pickDirectory({ defaultPath: draft.autoSaveDir || draft.lastDir || undefined });
    if (d) draft.autoSaveDir = d;
  }

  async function pickModule(v: (typeof CARD_VENDORS)[number]) {
    const ext = app.info?.os === "windows" ? ["dll"] : app.info?.os === "macos" ? ["dylib", "so"] : ["so"];
    const p = await api.openFileDialog({ filters: [{ name: "PKCS#11", extensions: ext }] });
    if (p) draft.pkcs11Paths[v] = p;
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && dirty) {
      e.preventDefault();
      void save();
    }
  }

  const themeOptions = $derived<{ value: Theme; label: string; icon: "monitor" | "sun" | "moon" }[]>([
    { value: "system", label: t("preference.theme.osDetermines"), icon: "monitor" },
    { value: "light", label: t("preference.theme.alwaysLight"), icon: "sun" },
    { value: "dark", label: t("preference.theme.alwaysDark"), icon: "moon" },
  ]);
  const autoSaveOptions = $derived<{ value: AutoSave; label: string }[]>([
    { value: "off", label: t("preference.autoSave.no") },
    { value: "save", label: t("preference.autoSave.save") },
    { value: "saveAndOpen", label: t("preference.autoSave.saveAndOpen") },
  ]);
  const modes = $derived<{ value: Mode; label: string }[]>([
    { value: "reader", label: t("preference.mode.e-documentReading") },
    { value: "xmlSign", label: t("preference.mode.xmlSigning") },
    { value: "smartbox", label: t("preference.mode.smartbox") },
  ]);

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

  let pdfScript = $derived<"latin" | "cyrillic">(draft.pdfCyrillicLabels ? "cyrillic" : "latin");
  $effect(() => {
    draft.pdfCyrillicLabels = pdfScript === "cyrillic";
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="page">
  <div class="page-header">
    <div>
      <h1>{t("preference.title")}</h1>
      <p>{t("settings.subtitle")}</p>
    </div>
  </div>

  <section class="card group">
    <h2><Icon name="sun" size={16} />{t("settings.appearance")}</h2>
    <div class="row">
      <div class="desc"><label for="set-lang">{t("preference.language")}</label></div>
      <div class="ctl">
        <div class="select-ic">
          <Icon name="globe" size={16} />
          <select id="set-lang" class="select" bind:value={draft.language}>
            {#each LANGUAGES as l (l)}<option value={l}>{languageNames[l]}</option>{/each}
          </select>
        </div>
      </div>
    </div>
    <div class="row">
      <div class="desc"><span>{t("preference.theme")}</span></div>
      <div class="ctl"><Segmented bind:value={draft.theme} options={themeOptions} label={t("preference.theme")} /></div>
    </div>
  </section>

  <section class="card group">
    <h2><Icon name="sliders" size={16} />{t("settings.application")}</h2>
    <div class="row">
      <div class="desc">
        <label for="set-mode">{t("preference.mode")}</label>
        <small>{t("settings.modeDesc")}</small>
      </div>
      <div class="ctl">
        <select id="set-mode" class="select" bind:value={draft.mode}>
          {#each modes as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
        </select>
      </div>
    </div>
    <div class="row">
      <div class="desc">
        <label for="set-bg">{t("preference.runInBackground")}</label>
        <small>{t("settings.runInBackgroundDesc")}</small>
      </div>
      <div class="ctl end">
        <Switch id="set-bg" bind:checked={draft.runInBackground} label={t("preference.runInBackground")} />
      </div>
    </div>
    <div class="row">
      <div class="desc">
        <label for="set-upd">{t("preference.checkVersionOnStartup")}</label>
        <small>{t("settings.checkUpdatesDesc")}</small>
      </div>
      <div class="ctl end">
        <Switch id="set-upd" bind:checked={draft.checkUpdates} label={t("preference.checkVersionOnStartup")} />
      </div>
    </div>
  </section>

  <section class="card group">
    <h2><Icon name="download" size={16} />{t("settings.documents")}</h2>
    <div class="row">
      <div class="desc">
        <span>{t("preference.pdfScript")}</span>
        <small>{t("settings.pdfScriptDesc")}</small>
      </div>
      <div class="ctl">
        <Segmented
          bind:value={pdfScript}
          label={t("preference.pdfScript")}
          options={[
            { value: "latin", label: t("settings.pdfLatin") },
            { value: "cyrillic", label: t("settings.pdfCyrillic") },
          ]}
        />
      </div>
    </div>
    <div class="row">
      <div class="desc">
        <span>{t("preference.autoSave")}</span>
        <small>{t("settings.autoSaveDesc")}</small>
      </div>
      <div class="ctl"><Segmented bind:value={draft.autoSave} options={autoSaveOptions} label={t("preference.autoSave")} /></div>
    </div>
    <div class="row" class:disabled={draft.autoSave === "off"}>
      <div class="desc"><label for="set-dir">{t("preference.autoSaveLocation")}</label></div>
      <div class="ctl">
        <div class="path-row">
          <input
            id="set-dir"
            class="input"
            placeholder={t("preference.placeholder.directoryPath")}
            bind:value={draft.autoSaveDir}
            disabled={draft.autoSave === "off"}
          />
          <button class="btn" type="button" onclick={pickDir} disabled={draft.autoSave === "off"}>
            <Icon name="folder" size={16} />{t("ui.browse")}
          </button>
        </div>
      </div>
    </div>
  </section>

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
</div>

{#if dirty}
  <div class="savebar" transition:fly={{ y: 20, duration: 200 }}>
    <span><Icon name="info" size={16} />{t("ui.unsaved")}</span>
    <div>
      <button class="btn btn-ghost" type="button" onclick={discard}>{t("ui.discard")}</button>
      <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
        <Icon name={saving ? "refresh" : "check"} size={16} class={saving ? "spin" : ""} />{t("ui.saveChanges")}
      </button>
    </div>
  </div>
{/if}

<style>
  .group {
    padding: 6px 20px 8px;
    margin-bottom: 16px;
  }
  .group h2 {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 14.5px;
    padding: 14px 0 8px;
    color: var(--text);
  }
  .group h2 :global(svg) {
    color: var(--accent);
  }
  .group-desc {
    font-size: 12.5px;
    color: var(--text-3);
    margin-bottom: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 480px);
    align-items: center;
    gap: 18px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  .group h2 + .row,
  .group-desc + .row {
    border-top: 0;
  }
  .row.disabled .desc {
    opacity: 0.55;
  }
  .desc {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .desc label,
  .desc span {
    font-weight: 550;
  }
  .desc small {
    font-size: 12.5px;
    color: var(--text-3);
  }
  .ctl {
    min-width: 0;
  }
  .ctl.end {
    display: flex;
    justify-content: flex-end;
  }
  .ctl.consent {
    align-items: center;
    gap: 8px;
  }
  .ctl :global(.seg) {
    flex-wrap: nowrap;
  }
  .select-ic {
    position: relative;
  }
  .select-ic :global(svg) {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-3);
    pointer-events: none;
  }
  .select-ic .select {
    padding-left: 34px;
  }
  .path-row {
    display: flex;
    gap: 8px;
  }
  .path-row .input {
    flex: 1;
    min-width: 0;
  }
  .input:disabled {
    opacity: 0.55;
  }
  .savebar {
    position: sticky;
    bottom: 16px;
    margin: 0 auto 16px;
    width: min(680px, calc(100% - 48px));
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 10px 10px 16px;
    border-radius: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    z-index: 5;
  }
  .savebar span {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    color: var(--text-2);
  }
  .savebar div {
    display: flex;
    gap: 8px;
  }
  @media (max-width: 900px) {
    .row {
      grid-template-columns: minmax(0, 1fr);
      gap: 8px;
    }
    .ctl.end {
      justify-content: flex-start;
    }
  }
</style>
