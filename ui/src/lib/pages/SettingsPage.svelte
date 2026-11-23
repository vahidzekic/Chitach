<script lang="ts">
  import { onDestroy } from "svelte";
  import { pro } from "$pro";
  import { fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Switch from "../components/Switch.svelte";
  import "./settings.css";
  import { api } from "../api";
  import { LANGUAGES, type AutoSave, type Mode, type Settings, type Theme } from "../api/types";
  import { languageNames, t } from "../i18n/index.svelte";
  import { app, isMobileOs, previewSettings, proAvailable, proUnlocked, saveSettings } from "../stores/app.svelte";
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

  // Phones: no tray, mode switch, auto-save folder or PKCS#11 modules.
  const isMobile = $derived(isMobileOs(app.info?.os));

  let pdfScript = $derived<"latin" | "cyrillic">(draft.pdfCyrillicLabels ? "cyrillic" : "latin");
  $effect(() => {
    draft.pdfCyrillicLabels = pdfScript === "cyrillic";
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="page settings">
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

  {#if !isMobile}
  <section class="card group">
    <h2><Icon name="sliders" size={16} />{t("settings.application")}</h2>
    {#if proAvailable()}
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
        {#if !proUnlocked()}<small class="pro-hint"><Icon name="lock" size={12} />{t("pro.lockedHint")}</small>{/if}
      </div>
      <div class="ctl end">
        <Switch
          id="set-bg"
          bind:checked={draft.runInBackground}
          label={t("preference.runInBackground")}
          disabled={!proUnlocked()}
        />
      </div>
    </div>
    {/if}
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
  {/if}

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
    {#if !isMobile && proAvailable()}
    <div class="row">
      <div class="desc">
        <span>{t("preference.autoSave")}</span>
        <small>{t("settings.autoSaveDesc")}</small>
        {#if !proUnlocked()}<small class="pro-hint"><Icon name="lock" size={12} />{t("pro.lockedHint")}</small>{/if}
      </div>
      <div class="ctl">
        <Segmented
          bind:value={draft.autoSave}
          options={autoSaveOptions}
          label={t("preference.autoSave")}
          disabled={!proUnlocked()}
        />
      </div>
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
    {/if}
  </section>

  {#if proAvailable() && pro.SettingsSections}
    <pro.SettingsSections bind:draft {isMobile} />
  {/if}
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
</style>
