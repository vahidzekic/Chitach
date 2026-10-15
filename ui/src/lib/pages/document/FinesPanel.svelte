<script lang="ts">
  // "Provera kazni": official portals for checking unpaid fines of the vehicle, with an
  // optional (opt-in) automatic check for providers that allow it.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { api } from "../../api";
  import type {
    FineItem,
    FinesCategory,
    FinesCheckResult,
    FinesInfo,
    FinesProvider,
    FinesStatus,
    Settings,
  } from "../../api/types";
  import { currentLanguage, hasKey, t } from "../../i18n/index.svelte";
  import { app, saveSettings, showError } from "../../stores/app.svelte";
  import { toast } from "../../stores/toasts.svelte";
  import { copyText } from "../../util/format";

  let info = $state<FinesInfo | null>(null);
  let results = $state<Record<string, FinesCheckResult>>({});
  let checking = $state<Record<string, boolean>>({});
  let expanded = $state<Record<string, boolean>>({});
  let showOthers = $state(false);
  let consentOpen = $state(false);
  let pendingForce = false;

  const CATEGORIES: FinesCategory[] = ["parking", "person", "toll"];

  const fines = $derived(app.settings?.fines ?? { autoCheck: false, consentGiven: false });
  const autoProviders = $derived(
    (info?.providers ?? []).filter(
      (p) => p.kind === "json_get" && (!fines.enabledProviders || fines.enabledProviders.includes(p.id)),
    ),
  );
  const anyChecking = $derived(Object.values(checking).some(Boolean));
  const anyResult = $derived(autoProviders.some((p) => results[p.id]));

  function providerName(p: FinesProvider): string {
    const key = `fines.provider.${p.id}.name`;
    return hasKey(key) ? t(key) : p.name;
  }
  function providerNote(p: FinesProvider): string | null {
    const key = `fines.provider.${p.id}.note`;
    return hasKey(key) ? t(key) : null;
  }

  /** Main list per category; parking portals of other cities are folded away. */
  function rows(cat: FinesCategory, all: boolean): FinesProvider[] {
    return (info?.providers ?? []).filter(
      (p) => p.category === cat && (cat !== "parking" || all || p.matchesCity || p.kind === "json_get"),
    );
  }
  const others = $derived(
    (info?.providers ?? []).filter((p) => p.category === "parking" && !p.matchesCity && p.kind !== "json_get"),
  );

  onMount(async () => {
    try {
      info = await api.finesProviders();
    } catch (e) {
      showError(e, "error.fines");
      return;
    }
    if (fines.autoCheck && fines.consentGiven && autoProviders.length > 0) void runChecks(false);
  });

  async function checkOne(id: string, force: boolean) {
    checking[id] = true;
    try {
      const r = await api.finesCheck(id, force);
      results[id] = r;
      if (r.status === "found") expanded[id] = true;
    } catch (e) {
      showError(e, "error.fines");
    } finally {
      checking[id] = false;
    }
  }

  async function runChecks(force: boolean) {
    await Promise.all(autoProviders.map((p) => checkOne(p.id, force)));
  }

  function requestCheck(force = false) {
    if (anyChecking) return;
    if (!fines.consentGiven) {
      pendingForce = force;
      consentOpen = true;
      return;
    }
    void runChecks(force);
  }

  async function allow() {
    const s = structuredClone($state.snapshot(app.settings)) as Settings;
    s.fines = { ...s.fines, consentGiven: true };
    consentOpen = false;
    if (await saveSettings(s)) void runChecks(pendingForce);
  }

  async function copyPlate(quiet = false) {
    if (!info?.normalizedPlate) return;
    if (await copyText(info.normalizedPlate)) {
      toast("success", t("fines.plateCopied", info.normalizedPlate), {
        detail: quiet ? t("fines.plateCopiedHint") : undefined,
      });
    }
  }

  async function open(p: FinesProvider) {
    // Portals without the plate in the URL: put it on the clipboard for pasting.
    if (!p.plateInUrl) await copyPlate(true);
    try {
      await api.openUrl(p.url);
    } catch (e) {
      showError(e, "error.openPath");
    }
  }

  const STATUS: Record<FinesStatus, { tone: string; icon: IconName }> = {
    clean: { tone: "success", icon: "checkCircle" },
    found: { tone: "danger", icon: "alert" },
    manual: { tone: "neutral", icon: "external" },
    unavailable: { tone: "warning", icon: "alertCircle" },
    parseError: { tone: "warning", icon: "alertCircle" },
  };

  function statusLabel(r: FinesCheckResult): string {
    return r.status === "found" ? t("fines.status.found", r.items.length) : t(`fines.status.${r.status}`);
  }

  function time(ms: number): string {
    return new Date(ms).toLocaleTimeString(currentLanguage(), { hour: "2-digit", minute: "2-digit" });
  }

  function parseAmount(s: string | null): number | null {
    if (!s) return null;
    let v = s.replace(/[^\d.,-]/g, "");
    if (v.includes(",") && v.includes(".")) v = v.lastIndexOf(",") > v.lastIndexOf(".") ? v.replace(/\./g, "").replace(",", ".") : v.replace(/,/g, "");
    else v = v.replace(",", ".");
    const n = Number(v);
    return v && Number.isFinite(n) ? n : null;
  }

  function money(n: number): string {
    return `${n.toLocaleString(currentLanguage().startsWith("sr") ? "sr-Latn-RS" : currentLanguage(), { minimumFractionDigits: 2, maximumFractionDigits: 2 })} RSD`;
  }

  function total(items: FineItem[]): string | null {
    const nums = items.map((i) => parseAmount(i.amount));
    return nums.every((n) => n !== null) ? money(nums.reduce((a, b) => a + (b ?? 0), 0)) : null;
  }

  /** Raw fields not already shown as date/amount/location/description. */
  function extra(i: FineItem): [string, string][] {
    const shown = new Set([i.date, i.amount, i.location, i.description]);
    return Object.entries(i.fields)
      .filter(([, v]) => !shown.has(v))
      .slice(0, 4);
  }

  const domains = $derived([...new Set(autoProviders.map((p) => p.checkDomain).filter(Boolean))].join(", "));
</script>

{#if info}
  <section class="fines card" aria-labelledby="fines-title">
    <header>
      <span class="ic"><Icon name="receipt" size={16} /></span>
      <div class="htext">
        <h2 id="fines-title">{t("fines.title")}</h2>
        <p>{t("fines.subtitle")}</p>
      </div>
      <div class="hactions">
        <span class="plate mono" title={t("vehicle.registrationNumberOfVehicle")}>{info.normalizedPlate || "—"}</span>
        <button class="btn btn-sm" type="button" onclick={() => copyPlate()} disabled={!info.normalizedPlate}>
          <Icon name="copy" size={14} />{t("fines.copyPlate")}
        </button>
        {#if autoProviders.length > 0}
          <button
            class="btn btn-sm"
            class:btn-primary={!anyResult}
            type="button"
            onclick={() => requestCheck(anyResult)}
            disabled={anyChecking || !info.normalizedPlate}
          >
            <Icon name={anyChecking ? "refresh" : anyResult ? "refresh" : "sparkle"} size={14} class={anyChecking ? "spin" : ""} />
            {anyResult ? t("fines.checkAgain") : t("fines.checkAuto")}
          </button>
        {/if}
      </div>
    </header>

    {#each CATEGORIES as cat (cat)}
      {@const list = rows(cat, showOthers)}
      {#if list.length > 0}
        <div class="cat">
          <h3>{t(`fines.category.${cat}`)}</h3>
          <ul>
            {#each list as p (p.id)}
              {@const r = results[p.id]}
              {@const note = providerNote(p)}
              <li class:match={p.matchesCity} transition:slide={{ duration: 160 }}>
                <div class="row">
                  <div class="main">
                    <div class="name">
                      <span>{providerName(p)}</span>
                      {#if p.matchesCity}
                        <span class="badge badge-accent tiny"><Icon name="car" size={12} stroke={2.2} />{t("fines.yourCity")}</span>
                      {/if}
                      {#if p.kind === "json_get"}
                        <span class="badge tiny"><Icon name="sparkle" size={12} stroke={2.2} />{t("fines.auto")}</span>
                      {/if}
                    </div>
                    {#if note}<p class="note">{note}</p>{/if}
                  </div>
                  <div class="side">
                    {#if p.kind === "json_get"}
                      {#if checking[p.id]}
                        <span class="badge"><Icon name="refresh" size={13} class="spin" />{t("fines.status.checking")}</span>
                      {:else if r}
                        {@const st = STATUS[r.status]}
                        <button
                          class="badge badge-{st.tone} status"
                          type="button"
                          disabled={r.status !== "found"}
                          aria-expanded={r.status === "found" ? !!expanded[p.id] : undefined}
                          onclick={() => (expanded[p.id] = !expanded[p.id])}
                        >
                          <Icon name={st.icon} size={13} stroke={2.2} />{statusLabel(r)}
                          {#if r.status === "found"}<Icon name={expanded[p.id] ? "chevronDown" : "chevronRight"} size={13} stroke={2.2} />{/if}
                        </button>
                      {:else}
                        <span class="badge">{t("fines.status.notChecked")}</span>
                      {/if}
                    {/if}
                    <button class="btn btn-sm" type="button" onclick={() => open(p)} title={p.url}>
                      <Icon name="external" size={14} />{t("fines.open")}
                    </button>
                  </div>
                </div>
                {#if r && !checking[p.id]}
                  <div class="meta">
                    {#if r.status !== "clean" && r.status !== "found"}{t(`fines.statusHint.${r.status}`)}{#if r.detail}<span class="detail mono"> ({r.detail})</span>{/if} ·
                    {/if}{t("fines.checkedAt", time(r.checkedAt))}{#if r.cached}&nbsp;· {t("fines.cached")}{/if}
                  </div>
                  {#if r.status === "found" && expanded[p.id]}
                    <div class="items" transition:slide={{ duration: 180 }}>
                      <table>
                        <thead>
                          <tr>
                            <th>{t("fines.item.date")}</th>
                            <th>{t("fines.item.location")}</th>
                            <th>{t("fines.item.description")}</th>
                            <th class="num">{t("fines.item.amount")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each r.items as it, i (i)}
                            {@const amt = parseAmount(it.amount)}
                            <tr>
                              <td class="nowrap">{it.date ?? "—"}</td>
                              <td>{it.location ?? "—"}</td>
                              <td>
                                {it.description ?? "—"}
                                {#each extra(it) as [k, v] (k)}<span class="kv"><span>{k}</span> <span class="mono">{v}</span></span>{/each}
                              </td>
                              <td class="num nowrap">{amt !== null ? money(amt) : (it.amount ?? "—")}</td>
                            </tr>
                          {/each}
                        </tbody>
                        {#if r.items.length > 1 && total(r.items)}
                          <tfoot>
                            <tr><td colspan="3">{t("fines.total")}</td><td class="num nowrap">{total(r.items)}</td></tr>
                          </tfoot>
                        {/if}
                      </table>
                      <p class="paynote">{t("fines.payOnPortal")}</p>
                    </div>
                  {/if}
                {/if}
              </li>
            {/each}
          </ul>
          {#if cat === "parking" && others.length > 0}
            <button class="more" type="button" onclick={() => (showOthers = !showOthers)} aria-expanded={showOthers}>
              <Icon name={showOthers ? "chevronDown" : "chevronRight"} size={14} stroke={2.2} />
              {showOthers
                ? t("fines.hideOtherCities")
                : t("fines.otherCities", others.map((p) => providerName(p).split(" – ")[0]).join(", "))}
            </button>
          {/if}
        </div>
      {/if}
    {/each}

    <footer>
      <Icon name="lock" size={14} />
      <span>{t("fines.privacy")} {t("fines.tip")}</span>
    </footer>
  </section>

  <Modal bind:open={consentOpen} title={t("fines.consent.title")}>
    <div class="consent">
      <p>{t("fines.consent.text", info.normalizedPlate)}</p>
      <ul class="domains">
        {#each autoProviders as p (p.id)}
          <li><Icon name="globe" size={14} /><span>{providerName(p)}</span><code>{p.checkDomain}</code></li>
        {/each}
      </ul>
      <p class="small">{t("fines.consent.details")}</p>
    </div>
    {#snippet actions()}
      <button class="btn btn-ghost" type="button" onclick={() => (consentOpen = false)}>{t("fines.consent.deny")}</button>
      <button class="btn btn-primary" type="button" onclick={allow} title={domains}>
        <Icon name="check" size={16} />{t("fines.consent.allow")}
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  .fines {
    padding: 16px 18px 12px;
    margin-bottom: 16px;
    animation: fade-up 0.35s var(--ease) both;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 6px;
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
  .plate {
    height: 30px;
    display: inline-flex;
    align-items: center;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1.5px solid var(--text);
    font-weight: 700;
    letter-spacing: 0.08em;
    background: var(--surface);
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
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  li + li {
    border-top: 1px solid var(--border);
  }
  li {
    padding: 10px 12px;
    background: var(--surface);
  }
  li.match {
    background: color-mix(in srgb, var(--accent-soft) 55%, var(--surface));
    box-shadow: inset 3px 0 0 var(--accent);
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
  }
  .badge.status {
    border: 0;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .badge.status:disabled {
    cursor: default;
  }
  .meta {
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-3);
  }
  .detail {
    font-size: 11px;
  }
  .items {
    margin-top: 8px;
    border-radius: var(--radius-sm);
    border: 1px solid color-mix(in srgb, var(--danger) 30%, var(--border));
    background: var(--surface);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th,
  td {
    text-align: left;
    padding: 7px 10px;
    vertical-align: top;
  }
  th {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
    background: var(--danger-soft);
  }
  tbody tr + tr td {
    border-top: 1px solid var(--border);
  }
  tfoot td {
    border-top: 1px solid var(--border);
    font-weight: 650;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .nowrap {
    white-space: nowrap;
  }
  .kv {
    display: block;
    font-size: 11.5px;
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
    margin-top: 6px;
    padding: 4px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--accent-text);
    font-size: 12.5px;
    font-weight: 550;
    cursor: pointer;
    text-align: left;
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
    .row {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
