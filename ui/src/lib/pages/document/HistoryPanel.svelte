<script lang="ts">
  // "Istorija vozila": what the card says, a local VIN check (nothing is sent), links to
  // history services and an optional report through an API with the user's own key.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { api } from "../../api";
  import type {
    HistoryApiResult,
    HistoryApiStatus,
    HistoryCategory,
    HistoryCheckLevel,
    HistoryInfo,
    HistoryProvider,
    Settings,
  } from "../../api/types";
  import { currentLanguage, hasKey, t } from "../../i18n/index.svelte";
  import { app, navigate, saveSettings, showError } from "../../stores/app.svelte";
  import { toast } from "../../stores/toasts.svelte";
  import { copyText } from "../../util/format";

  let info = $state<HistoryInfo | null>(null);
  let results = $state<Record<string, HistoryApiResult>>({});
  let running = $state<Record<string, boolean>>({});
  let showUsa = $state(false);
  let consentOpen = $state(false);
  let pending: { provider: HistoryProvider; op: string; force: boolean } | null = null;

  const consentGiven = $derived(app.settings?.history?.consentGiven ?? false);
  const decoded = $derived(info?.decoded ?? null);

  onMount(async () => {
    try {
      info = await api.historyInfo();
    } catch (e) {
      showError(e, "error.history");
    }
  });

  function tr(key: string, fallback: string, ...args: (string | number)[]): string {
    return hasKey(key) ? t(key, ...args) : fallback;
  }
  const providerName = (p: HistoryProvider) => tr(`history.provider.${p.id}.name`, p.name);
  const providerNote = (p: HistoryProvider) => tr(`history.provider.${p.id}.note`, "");
  const opName = (op: string) => tr(`history.op.${op}`, op);

  function country(code: string | null): string | null {
    if (!code) return null;
    try {
      const lang = currentLanguage() === "sr-Cyrl" ? "sr-Cyrl" : currentLanguage() === "sr-Latn" ? "sr-Latn" : currentLanguage();
      return new Intl.DisplayNames([lang, "en"], { type: "region" }).of(code) ?? code;
    } catch {
      return code;
    }
  }

  function rows(cat: HistoryCategory): HistoryProvider[] {
    return (info?.providers ?? []).filter((p) => p.category === cat);
  }

  const LEVEL: Record<HistoryCheckLevel, { tone: string; icon: IconName }> = {
    ok: { tone: "success", icon: "checkCircle" },
    warning: { tone: "warning", icon: "alert" },
    info: { tone: "accent", icon: "info" },
    unknown: { tone: "neutral", icon: "alertCircle" },
  };

  const STATUS: Record<HistoryApiStatus, { tone: string; icon: IconName }> = {
    ok: { tone: "success", icon: "checkCircle" },
    unauthorized: { tone: "danger", icon: "key" },
    error: { tone: "warning", icon: "alertCircle" },
    unavailable: { tone: "warning", icon: "alertCircle" },
    parseError: { tone: "warning", icon: "alertCircle" },
  };

  async function copy(value: string, label: string, hint = false) {
    if (!value) return;
    if (await copyText(value)) {
      toast("success", t("history.copied", label, value), { detail: hint ? t("history.copiedHint") : undefined });
    }
  }
  const copyVin = (hint = false) => copy(info?.normalizedVin ?? "", "VIN", hint);
  const copyPlate = (hint = false) => copy(info?.normalizedPlate ?? "", t("history.plate"), hint);

  async function open(p: HistoryProvider, url = p.url) {
    if (!p.keyInUrl) await (p.key === "plate" ? copyPlate(true) : copyVin(true));
    try {
      await api.openUrl(url);
    } catch (e) {
      showError(e, "error.openPath");
    }
  }

  const rkey = (p: HistoryProvider, op: string) => `${p.id}|${op}`;

  async function runOp(p: HistoryProvider, op: string, force: boolean) {
    const k = rkey(p, op);
    running[k] = true;
    try {
      results[k] = await api.historyApiCheck(p.id, op, force);
    } catch (e) {
      showError(e, "error.history");
    } finally {
      running[k] = false;
    }
  }

  function request(p: HistoryProvider, op: string, force = false) {
    if (running[rkey(p, op)]) return;
    if (!consentGiven) {
      pending = { provider: p, op, force };
      consentOpen = true;
      return;
    }
    void runOp(p, op, force);
  }

  async function allow() {
    const s = structuredClone($state.snapshot(app.settings)) as Settings;
    s.history = { ...s.history, consentGiven: true };
    consentOpen = false;
    if ((await saveSettings(s)) && pending) void runOp(pending.provider, pending.op, pending.force);
    pending = null;
  }

  function time(ms: number): string {
    return new Date(ms).toLocaleTimeString(currentLanguage(), { hour: "2-digit", minute: "2-digit" });
  }

  const apiProviders = $derived(rows("api"));
  const facts = $derived.by(() => {
    if (!info) return [];
    return [
      { label: t("vehicle.yearOfProduction"), value: info.yearOfProduction },
      { label: t("vehicle.dateOfFirstRegistration"), value: info.dateOfFirstRegistration },
      {
        label: t("history.vinMake"),
        value: decoded ? [decoded.make, country(decoded.country)].filter(Boolean).join(" · ") || "—" : "—",
      },
      { label: t("history.vinModelYear"), value: decoded?.modelYear ? String(decoded.modelYear) : "—" },
    ];
  });
</script>

{#if info}
  <section class="history card" aria-labelledby="history-title">
    <header>
      <span class="ic"><Icon name="history" size={16} /></span>
      <div class="htext">
        <h2 id="history-title">{t("history.title")}</h2>
        <p>{t("history.subtitle")}</p>
      </div>
      <div class="hactions">
        <span class="vin mono" title="VIN">{info.normalizedVin || "—"}</span>
        <button class="btn btn-sm" type="button" onclick={() => copyVin()} disabled={!info.normalizedVin}>
          <Icon name="copy" size={14} />{t("history.copyVin")}
        </button>
      </div>
    </header>

    <dl class="facts">
      {#each facts as f (f.label)}
        <div><dt>{f.label}</dt><dd>{f.value || "—"}</dd></div>
      {/each}
    </dl>

    <div class="cat">
      <h3>{t("history.checksTitle")}</h3>
      <ul class="checks">
        {#each info.checks as c (c.id)}
          {@const lv = LEVEL[c.level]}
          <li class="tone-{lv.tone}">
            <Icon name={lv.icon} size={15} stroke={2.2} />
            <span>{tr(`history.check.${c.id}.${c.variant}`, `${c.id}: ${c.variant}`, ...c.args)}</span>
          </li>
        {/each}
      </ul>
    </div>

    {#each ["domestic", "foreign"] as const as cat (cat)}
      {@const list = rows(cat)}
      {#if list.length > 0}
        <div class="cat">
          <h3>{t(`history.category.${cat}`)}</h3>
          <ul class="list">
            {#each list as p (p.id)}
              {@const note = providerNote(p)}
              <li>
                <div class="row">
                  <div class="main">
                    <div class="name">
                      <span>{providerName(p)}</span>
                      {#if p.price}
                        <span class="badge tiny">{p.price === "0" ? t("history.free") : t("history.price", p.price)}</span>
                      {/if}
                      {#if p.partner}<span class="badge tiny badge-warning">{t("history.partner")}</span>{/if}
                    </div>
                    {#if note}<p class="note">{note}</p>{/if}
                  </div>
                  <div class="side">
                    <button class="btn btn-sm" type="button" onclick={() => open(p)} title={p.url}>
                      <Icon name="external" size={14} />{p.altUrl ? t(`history.provider.${p.id}.main`) : t("history.open")}
                    </button>
                    {#if p.altUrl}
                      <button class="btn btn-sm" type="button" onclick={() => open(p, p.altUrl ?? p.url)} title={p.altUrl}>
                        <Icon name="external" size={14} />{t(`history.provider.${p.id}.alt`)}
                      </button>
                    {/if}
                  </div>
                </div>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    {/each}

    {#if rows("usa").length > 0}
      {@const usaOpen = showUsa || !!decoded?.northAmerica}
      <div class="cat">
        {#if usaOpen}
          <h3>{t("history.category.usa")}</h3>
          <ul class="list" transition:slide={{ duration: 160 }}>
            {#each rows("usa") as p (p.id)}
              {@const note = providerNote(p)}
              <li>
                <div class="row">
                  <div class="main">
                    <div class="name">
                      <span>{providerName(p)}</span>
                      {#if p.price}
                        <span class="badge tiny">{p.price === "0" ? t("history.free") : t("history.price", p.price)}</span>
                      {/if}
                    </div>
                    {#if note}<p class="note">{note}</p>{/if}
                  </div>
                  <div class="side">
                    <button class="btn btn-sm" type="button" onclick={() => open(p)} title={p.url}>
                      <Icon name="external" size={14} />{t("history.open")}
                    </button>
                  </div>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
        {#if !decoded?.northAmerica}
          <button class="more" type="button" onclick={() => (showUsa = !showUsa)} aria-expanded={showUsa}>
            <Icon name={showUsa ? "chevronDown" : "chevronRight"} size={14} stroke={2.2} />
            {showUsa ? t("history.hideUsa") : t("history.showUsa")}
          </button>
        {/if}
      </div>
    {/if}

    {#if apiProviders.length > 0}
      <div class="cat">
        <h3>{t("history.category.api")}</h3>
        <ul class="list">
          {#each apiProviders as p (p.id)}
            <li>
              <div class="row">
                <div class="main">
                  <div class="name">
                    <span>{providerName(p)}</span>
                    <span class="badge tiny"><Icon name="key" size={12} stroke={2.2} />{t("history.byok")}</span>
                  </div>
                  <p class="note">
                    {p.configured ? t("history.apiConfigured", p.apiDomain ?? "") : t("history.apiNotConfigured")}
                  </p>
                </div>
                <div class="side">
                  {#if p.configured}
                    {#each p.operations as op (op)}
                      {@const k = rkey(p, op)}
                      <button
                        class="btn btn-sm"
                        type="button"
                        onclick={() => request(p, op, !!results[k])}
                        disabled={running[k] || info.vinFormat !== "valid"}
                      >
                        <Icon name={running[k] ? "refresh" : "sparkle"} size={14} class={running[k] ? "spin" : ""} />{opName(op)}
                      </button>
                    {/each}
                  {:else}
                    <button class="btn btn-sm" type="button" onclick={() => navigate("settings")}>
                      <Icon name="sliders" size={14} />{t("history.addKey")}
                    </button>
                  {/if}
                </div>
              </div>
              {#each p.operations as op (op)}
                {@const r = results[rkey(p, op)]}
                {#if r && !running[rkey(p, op)]}
                  {@const st = STATUS[r.status]}
                  <div class="result" transition:slide={{ duration: 180 }}>
                    <div class="rhead">
                      <span class="badge badge-{st.tone}"><Icon name={st.icon} size={13} stroke={2.2} />{opName(op)}: {t(`history.status.${r.status}`)}</span>
                      <span class="meta">
                        {t("history.checkedAt", time(r.checkedAt))}{#if r.price}&nbsp;· {t("history.charged", r.price)}{/if}{#if r.cached}&nbsp;· {t("history.cached")}{/if}{#if r.detail}<span class="mono">&nbsp;({r.detail})</span>{/if}
                      </span>
                    </div>
                    {#if r.fields.length > 0}
                      <table>
                        <tbody>
                          {#each r.fields as [k, v], i (i)}
                            <tr><th>{k}</th><td>{v}</td></tr>
                          {/each}
                        </tbody>
                      </table>
                      <p class="paynote">{t("history.providerData", providerName(p))}</p>
                    {/if}
                  </div>
                {/if}
              {/each}
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    <footer>
      <Icon name="lock" size={14} />
      <span>{t("history.privacy")} {t("history.mileageNote")}</span>
    </footer>
  </section>

  <Modal bind:open={consentOpen} title={t("history.consent.title")}>
    <div class="consent">
      <p>{t("history.consent.text", info.normalizedVin)}</p>
      <ul class="domains">
        {#each apiProviders.filter((p) => p.configured) as p (p.id)}
          <li><Icon name="globe" size={14} /><span>{providerName(p)}</span><code>{p.apiDomain}</code></li>
        {/each}
      </ul>
      <p class="small">{t("history.consent.details")}</p>
    </div>
    {#snippet actions()}
      <button class="btn btn-ghost" type="button" onclick={() => (consentOpen = false)}>{t("fines.consent.deny")}</button>
      <button class="btn btn-primary" type="button" onclick={allow}>
        <Icon name="check" size={16} />{t("fines.consent.allow")}
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  .history {
    padding: 16px 18px 12px;
    margin-bottom: 16px;
    animation: fade-up 0.35s var(--ease) both;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 10px;
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
  .htext p {
    font-size: 12.5px;
    color: var(--text-3);
  }
  .hactions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .vin {
    height: 30px;
    display: inline-flex;
    align-items: center;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    font-weight: 650;
    letter-spacing: 0.04em;
    background: var(--surface);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin: 0;
  }
  .facts div {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }
  dt {
    font-size: 11.5px;
    color: var(--text-3);
  }
  dd {
    margin: 2px 0 0;
    font-weight: 600;
  }
  .cat {
    margin-top: 12px;
  }
  h3 {
    font-size: 11.5px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
    margin-bottom: 6px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .checks li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 3px 0;
    font-size: 13px;
  }
  .checks li :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .tone-success :global(svg) {
    color: var(--success);
  }
  .tone-warning :global(svg) {
    color: var(--warning);
  }
  .tone-accent :global(svg) {
    color: var(--accent-text);
  }
  .tone-neutral :global(svg) {
    color: var(--text-3);
  }
  .list {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .list > li + li {
    border-top: 1px solid var(--border);
  }
  .list > li {
    padding: 10px 12px;
    background: var(--surface);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .main {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-weight: 600;
  }
  .badge.tiny {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
    gap: 4px;
  }
  .note {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .side {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .result {
    margin-top: 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    overflow-x: auto;
  }
  .rhead {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 8px;
  }
  .meta {
    font-size: 12px;
    color: var(--text-3);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th,
  td {
    text-align: left;
    padding: 5px 10px;
    vertical-align: top;
    border-top: 1px solid var(--border);
  }
  th {
    width: 40%;
    font-weight: 550;
    color: var(--text-3);
  }
  .paynote {
    padding: 6px 10px 8px;
    font-size: 12px;
    color: var(--text-3);
    border-top: 1px dashed var(--border);
  }
  .more {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    padding: 4px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--accent-text);
    font-size: 12.5px;
    font-weight: 550;
    cursor: pointer;
  }
  .more:hover {
    background: var(--accent-soft);
  }
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px dashed var(--border);
    font-size: 12.5px;
    color: var(--text-3);
  }
  footer :global(svg) {
    margin-top: 2px;
  }
  .consent p {
    color: var(--text-2);
  }
  .consent .small {
    font-size: 12.5px;
    color: var(--text-3);
  }
  .domains {
    margin: 10px 0 12px;
  }
  .domains li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    font-weight: 550;
  }
  .domains code {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--accent-text);
  }
  @media (max-width: 760px) {
    .facts {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .row {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
