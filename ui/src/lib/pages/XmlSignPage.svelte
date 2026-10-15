<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { api } from "../api";
  import type { CardVendor, Slot, TokenCert } from "../api/types";
  import { t, translateError } from "../i18n/index.svelte";
  import { navigate, openPath, saveFileAs, showError } from "../stores/app.svelte";
  import { toast } from "../stores/toasts.svelte";
  import { baseName, formatCertDate } from "../util/format";

  type Step = 0 | 1 | 2 | 3 | 4 | 5;

  let step = $state<Step>(0);
  let loading = $state(false);
  let modules = $state<CardVendor[] | null>(null);
  let moduleError = $state<string | null>(null);
  let vendor = $state<CardVendor | null>(null);
  let slots = $state<Slot[] | null>(null);
  let slot = $state<number | null>(null);
  let pin = $state("");
  let showPin = $state(false);
  let loginError = $state<string | null>(null);
  let certs = $state<TokenCert[]>([]);
  let certId = $state<string | null>(null);
  let inputPath = $state("");
  let outputPath = $state("");
  let loggedIn = false;

  const steps = $derived([
    { label: t("xmlSign.module"), title: t("xmlSign.step.selectModule") },
    { label: t("xmlSign.slot"), title: t("xmlSign.step.selectSlot") },
    { label: "PIN", title: t("xmlSign.step.login") },
    { label: t("xmlSign.certificate"), title: t("xmlSign.step.selectCert") },
    { label: t("xmlSign.file"), title: t("xmlSign.step.openXml") },
  ]);

  async function loadModules() {
    loading = true;
    moduleError = null;
    try {
      modules = await api.signLoadModules();
      if (modules.length === 1) vendor = modules[0];
    } catch (e) {
      modules = [];
      moduleError = translateError(e).detail ?? translateError(e).title;
    } finally {
      loading = false;
    }
  }

  async function loadSlots() {
    if (!vendor) return;
    loading = true;
    slots = null;
    slot = null;
    try {
      slots = await api.signListSlots(vendor);
      if (slots.length === 1) slot = slots[0].id;
    } catch (e) {
      slots = [];
      showError(e);
    } finally {
      loading = false;
    }
  }

  async function login() {
    if (!vendor || slot === null || !pin) return;
    loading = true;
    loginError = null;
    try {
      certs = await api.signLogin(vendor, slot, pin);
      loggedIn = true;
      certId = certs.length > 0 ? certs[0].id : null;
      pin = "";
      step = 3;
    } catch (e) {
      const { title, detail } = translateError(e);
      loginError = title === t("error.error") ? t("xmlSign.loginError") : title;
      if (detail && title !== t("error.error")) loginError += ` (${detail})`;
    } finally {
      loading = false;
    }
  }

  async function logout() {
    if (!loggedIn) return;
    loggedIn = false;
    try {
      await api.signLogout();
    } catch {
      /* ignore */
    }
  }

  async function chooseInput() {
    const p = await api.openFileDialog({ filters: [{ name: "XML", extensions: ["xml"] }] });
    if (!p) return;
    inputPath = p;
    outputPath = p.replace(/(\.xml)?$/i, "_signed.xml");
  }

  async function chooseOutput() {
    const name = baseName(outputPath || "signed.xml");
    const p = await saveFileAs(name, "xml", "XML");
    if (p) outputPath = p;
  }

  async function sign() {
    if (!certId || !inputPath || !outputPath) return;
    loading = true;
    try {
      await api.signXmlFile(certId, inputPath, outputPath);
      toast("success", t("xmlSign.xmlSaved"), {
        detail: baseName(outputPath),
        action: { label: t("ui.open"), run: () => void openPath(outputPath) },
      });
      step = 5;
    } catch (e) {
      showError(e, "xmlSign.signError");
    } finally {
      loading = false;
    }
  }

  async function next() {
    if (step === 0 && vendor) {
      step = 1;
      await loadSlots();
    } else if (step === 1 && slot !== null) {
      step = 2;
    } else if (step === 2) {
      await login();
    } else if (step === 3 && certId) {
      step = 4;
    }
  }

  async function back() {
    if (step === 3) await logout();
    if (step > 0) step = (step - 1) as Step;
    if (step === 2) loginError = null;
  }

  async function restart() {
    await logout();
    inputPath = outputPath = "";
    certs = [];
    certId = null;
    step = 0;
    await loadModules();
  }

  const canNext = $derived(
    !loading &&
      ((step === 0 && !!vendor) ||
        (step === 1 && slot !== null) ||
        (step === 2 && pin.length > 0) ||
        (step === 3 && !!certId)),
  );

  onMount(loadModules);
  onDestroy(() => void logout());
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>{t("xmlSign.title")}</h1>
      <p>{t("xmlSign.subtitle")}</p>
    </div>
    {#if step > 0}
      <button class="btn btn-ghost" type="button" onclick={restart}>
        <Icon name="refresh" size={16} />{t("xmlSign.startOver")}
      </button>
    {/if}
  </div>

  <ol class="stepper" aria-label={t("xmlSign.title")}>
    {#each steps as s, i (i)}
      <li class:done={i < step} class:current={i === step} aria-current={i === step ? "step" : undefined}>
        <span class="num">
          {#if i < step}<Icon name="check" size={14} stroke={2.6} />{:else}{i + 1}{/if}
        </span>
        <span class="lbl">{s.label}</span>
      </li>
    {/each}
  </ol>

  <section class="card panel">
    {#if step < 5}
      <h2>{steps[step].title}</h2>
    {/if}

    {#if step === 0}
      {#if modules === null}
        <p class="muted"><Icon name="refresh" size={15} class="spin" /> {t("ui.loading")}</p>
      {:else if modules.length === 0}
        <div class="notice">
          <Icon name="alert" size={20} />
          <div>
            <strong>{t("xmlSign.noModules")}</strong>
            <p>{t("xmlSign.noModulesExplanation")}</p>
            {#if moduleError}<p class="mono small">{moduleError}</p>{/if}
          </div>
        </div>
        <div class="row-actions">
          <button class="btn" type="button" onclick={loadModules}><Icon name="refresh" size={16} />{t("xmlSign.refresh")}</button>
          <button class="btn btn-primary" type="button" onclick={() => navigate("settings")}>
            <Icon name="sliders" size={16} />{t("ui.goToSettings")}
          </button>
        </div>
      {:else}
        <div class="options" role="radiogroup" aria-label={t("xmlSign.module")}>
          {#each modules as m (m)}
            <label class="option" class:selected={vendor === m}>
              <input type="radio" name="vendor" value={m} bind:group={vendor} />
              <span class="opt-ic"><Icon name="chip" size={18} /></span>
              <span class="opt-text">
                <strong>{t(`vendor.${m}`)}</strong>
                <span class="faint mono">{m}</span>
              </span>
              <span class="radio"></span>
            </label>
          {/each}
        </div>
      {/if}
    {:else if step === 1}
      {#if slots === null}
        <p class="muted"><Icon name="refresh" size={15} class="spin" /> {t("ui.loading")}</p>
      {:else if slots.length === 0}
        <div class="notice"><Icon name="alert" size={20} /><div><p>{t("xmlSign.noSlots")}</p></div></div>
      {:else}
        <div class="options" role="radiogroup" aria-label={t("xmlSign.slot")}>
          {#each slots as s (s.id)}
            <label class="option" class:selected={slot === s.id}>
              <input type="radio" name="slot" value={s.id} bind:group={slot} />
              <span class="opt-ic"><Icon name="reader" size={18} /></span>
              <span class="opt-text">
                <strong>{s.description}</strong>
                <span class="faint">{t("xmlSign.slot")} {s.id}</span>
              </span>
              <span class="radio"></span>
            </label>
          {/each}
        </div>
      {/if}
      <div class="row-actions">
        <button class="btn btn-sm" type="button" onclick={loadSlots} disabled={loading}>
          <Icon name="refresh" size={14} />{t("xmlSign.refresh")}
        </button>
      </div>
    {:else if step === 2}
      <form
        class="pin-form"
        onsubmit={(e) => {
          e.preventDefault();
          void login();
        }}
      >
        <label class="label" for="sign-pin">PIN</label>
        <div class="pin-row">
          <input
            id="sign-pin"
            class="input mono"
            type={showPin ? "text" : "password"}
            autocomplete="off"
            bind:value={pin}
            aria-invalid={!!loginError}
          />
          <button
            class="btn btn-icon"
            type="button"
            onclick={() => (showPin = !showPin)}
            aria-label={showPin ? t("ui.hidePin") : t("ui.showPin")}
            aria-pressed={showPin}
          >
            <Icon name={showPin ? "eyeOff" : "eye"} size={16} />
          </button>
        </div>
        <p class="warn"><Icon name="alert" size={14} />{t("xmlSign.pinWarning")}</p>
        {#if loginError}<div class="error" role="alert"><Icon name="alertCircle" size={16} />{loginError}</div>{/if}
      </form>
    {:else if step === 3}
      {#if certs.length === 0}
        <div class="notice"><Icon name="alert" size={20} /><div><p>{t("xmlSign.noCerts")}</p></div></div>
      {:else}
        <div class="options" role="radiogroup" aria-label={t("xmlSign.certificate")}>
          {#each certs as c (c.id)}
            <label class="option" class:selected={certId === c.id}>
              <input type="radio" name="cert" value={c.id} bind:group={certId} />
              <span class="opt-ic"><Icon name="shield" size={18} /></span>
              <span class="opt-text">
                <strong>{c.label}</strong>
                <span class="faint">{c.subject}</span>
                <span class="faint">{t("certs.validTo")}: {formatCertDate(c.notAfter)}</span>
              </span>
              <span class="radio"></span>
            </label>
          {/each}
        </div>
      {/if}
    {:else if step === 4}
      <div class="files">
        <div>
          <span class="label">{t("xmlSign.inputFile")}</span>
          <div class="file-row">
            <div class="path" class:empty={!inputPath}>
              <Icon name="fileCode" size={17} />
              <span title={inputPath}>{inputPath || "—"}</span>
            </div>
            <button class="btn" type="button" onclick={chooseInput}>{t("xmlSign.chooseFile")}</button>
          </div>
        </div>
        <div>
          <span class="label">{t("xmlSign.outputFile")}</span>
          <div class="file-row">
            <div class="path" class:empty={!outputPath}>
              <Icon name="file" size={17} />
              <span title={outputPath}>{outputPath || "—"}</span>
            </div>
            <button class="btn" type="button" onclick={chooseOutput} disabled={!inputPath}>{t("ui.browse")}</button>
          </div>
        </div>
      </div>
    {:else}
      <div class="done">
        <div class="done-ic"><Icon name="check" size={30} stroke={2.4} /></div>
        <h2>{t("xmlSign.done")}</h2>
        <p class="muted mono small">{outputPath}</p>
        <div class="row-actions center">
          <button class="btn" type="button" onclick={() => void openPath(outputPath)}>
            <Icon name="external" size={16} />{t("ui.open")}
          </button>
          <button
            class="btn btn-primary"
            type="button"
            onclick={() => {
              inputPath = outputPath = "";
              step = 4;
            }}
          >
            <Icon name="pen" size={16} />{t("xmlSign.signAnother")}
          </button>
        </div>
      </div>
    {/if}

    {#if step < 5 && !(step === 0 && modules?.length === 0)}
      <div class="nav">
        <button class="btn btn-ghost" type="button" onclick={back} disabled={step === 0 || loading}>
          <Icon name="chevronLeft" size={16} />{t("xmlSign.back")}
        </button>
        {#if step < 4}
          <button class="btn btn-primary" type="button" onclick={next} disabled={!canNext}>
            {#if loading}<Icon name="refresh" size={15} class="spin" />{/if}
            {step === 2 && loading ? t("xmlSign.loggingIn") : t("xmlSign.next")}
            <Icon name="chevronRight" size={16} />
          </button>
        {:else}
          <button class="btn btn-primary" type="button" onclick={sign} disabled={loading || !inputPath || !outputPath}>
            <Icon name={loading ? "refresh" : "pen"} size={16} class={loading ? "spin" : ""} />
            {loading ? t("xmlSign.signing") : t("xmlSign.sign")}
          </button>
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .stepper {
    list-style: none;
    display: flex;
    gap: 6px;
    margin: 0 0 16px;
    padding: 0;
    counter-reset: s;
  }
  .stepper li {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-3);
    font-size: 12.5px;
    font-weight: 600;
  }
  .num {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    font-size: 11.5px;
  }
  .lbl {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .stepper li.current {
    border-color: var(--accent);
    color: var(--accent-text);
    box-shadow: var(--focus);
  }
  .stepper li.current .num {
    background: var(--accent);
    color: var(--accent-contrast);
  }
  .stepper li.done {
    color: var(--text-2);
  }
  .stepper li.done .num {
    background: var(--success);
    color: #fff;
  }
  .panel {
    padding: 22px;
  }
  .panel > h2 {
    font-size: 16px;
    margin-bottom: 16px;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .option {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .option:hover {
    border-color: var(--border-strong);
    background: var(--surface-2);
  }
  .option.selected {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .option:focus-within {
    box-shadow: var(--focus);
  }
  .option input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .opt-ic {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    color: var(--text-2);
  }
  .selected .opt-ic {
    background: var(--accent);
    color: var(--accent-contrast);
  }
  .opt-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .opt-text .faint {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .radio {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
  }
  .selected .radio {
    border: 5px solid var(--accent);
  }
  .nav {
    display: flex;
    justify-content: space-between;
    margin-top: 22px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .row-actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .row-actions.center {
    justify-content: center;
  }
  .notice {
    display: flex;
    gap: 12px;
    padding: 14px;
    border-radius: var(--radius);
    background: var(--warning-soft);
    color: var(--warning-text);
  }
  .notice p {
    color: var(--text-2);
    margin-top: 2px;
  }
  .small {
    font-size: 12px;
  }
  .pin-form {
    max-width: 360px;
  }
  .pin-row {
    display: flex;
    gap: 8px;
  }
  .pin-row .input {
    letter-spacing: 0.2em;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    font-size: 12.5px;
    color: var(--warning-text);
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger-text);
    font-weight: 550;
    font-size: 13px;
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .file-row {
    display: flex;
    gap: 8px;
  }
  .path {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-size: 13px;
  }
  .path span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .path.empty {
    color: var(--text-3);
  }
  .done {
    text-align: center;
    padding: 12px 0;
  }
  .done-ic {
    width: 64px;
    height: 64px;
    margin: 0 auto 14px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--success-soft);
    color: var(--success);
  }
  .done h2 {
    font-size: 18px;
    margin-bottom: 6px;
  }
  .muted :global(.spin) {
    display: inline-block;
    vertical-align: -3px;
  }
  @media (max-width: 760px) {
    .stepper .lbl {
      display: none;
    }
    .stepper li {
      justify-content: center;
    }
  }
</style>
