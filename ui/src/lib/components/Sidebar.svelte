<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";
  import Logo from "./Logo.svelte";
  import { api } from "../api";
  import type { CardStatus } from "../api/types";
  import { t } from "../i18n/index.svelte";
  import { app, navigate, showError, type Page } from "../stores/app.svelte";

  interface NavItem {
    page: Page;
    label: string;
    icon: IconName;
  }

  const mode = $derived(app.mode);
  const isId = $derived(app.state.document?.kind === "id");

  const mainItems = $derived.by<NavItem[]>(() => {
    if (mode === "xmlSign") return [{ page: "xmlSign", label: t("nav.xmlSign"), icon: "pen" }];
    if (mode === "smartbox") return [{ page: "smartbox", label: t("nav.smartbox"), icon: "server" }];
    const items: NavItem[] = [{ page: "document", label: t("nav.document"), icon: docIcon() }];
    if (isId) items.push({ page: "certificates", label: t("nav.certificates"), icon: "shield" });
    return items;
  });

  const bottomItems = $derived<NavItem[]>([
    { page: "settings", label: t("nav.settings"), icon: "sliders" },
    { page: "about", label: t("nav.about"), icon: "info" },
  ]);

  function docIcon(): IconName {
    const k = app.state.document?.kind;
    return k === "medical" ? "medical" : k === "vehicle" ? "car" : "card";
  }

  // If the certificates page disappears (card removed / other card), go back.
  $effect(() => {
    if (app.page === "certificates" && !isId && app.state.status !== "reading") navigate("document");
  });

  const statusTone: Record<CardStatus, string> = {
    ready: "ok",
    reading: "busy",
    waiting: "idle",
    noReader: "bad",
    unknownCard: "warn",
    error: "bad",
  };
  const statusLabel: Record<CardStatus, string> = {
    ready: "status.ready",
    reading: "status.reading",
    waiting: "status.waiting",
    noReader: "status.noReader",
    unknownCard: "status.unknownCard",
    error: "status.error",
  };

  async function onReaderChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    try {
      await api.selectReader(v === "" ? null : v);
    } catch (err) {
      showError(err);
    }
  }
</script>

<aside class="sidebar">
  <div class="brand">
    <Logo />
    <div>
      <div class="name">Chitach</div>
      <div class="tag">{t("app.tagline")}</div>
    </div>
  </div>

  {#if mode === "reader"}
    <div class="reader">
      <label class="label" for="reader-select">{t("ui.reader")}</label>
      <div class="select-wrap">
        <Icon name="reader" size={16} class="reader-ic" />
        <select
          id="reader-select"
          class="select"
          value={app.state.selectedReader ?? ""}
          onchange={onReaderChange}
          disabled={app.state.readers.length === 0}
          title={app.state.selectedReader ?? t("reader.auto")}
        >
          {#if app.state.readers.length === 0}
            <option value="">{t("reader.none")}</option>
          {:else}
            <option value="">{t("reader.auto")}</option>
            {#each app.state.readers as r (r.name)}
              <option value={r.name}>{r.name} — {r.cardPresent ? t("reader.cardPresent") : t("reader.noCard")}</option>
            {/each}
          {/if}
        </select>
      </div>
      <div class="status" role="status" aria-live="polite">
        <span class="dot {statusTone[app.state.status]}"></span>
        {t(statusLabel[app.state.status])}
      </div>
    </div>
  {/if}

  <nav aria-label={t("nav.main")}>
    <ul>
      {#each mainItems as item (item.page)}
        <li>
          <button
            type="button"
            class="nav-item"
            class:active={app.page === item.page}
            aria-current={app.page === item.page ? "page" : undefined}
            onclick={() => navigate(item.page)}
          >
            <Icon name={item.icon} size={18} />
            <span>{item.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="spacer"></div>

  {#if api.isMock}
    <div class="demo" title={t("demo.desc")}>
      <Icon name="sparkle" size={14} />
      <span>{t("demo.badge")}</span>
    </div>
  {/if}

  <nav aria-label={t("nav.settings")}>
    <ul>
      {#each bottomItems as item (item.page)}
        <li>
          <button
            type="button"
            class="nav-item"
            class:active={app.page === item.page}
            aria-current={app.page === item.page ? "page" : undefined}
            onclick={() => navigate(item.page)}
          >
            <Icon name={item.icon} size={18} />
            <span>{item.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-w);
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 18px 14px 14px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 2px 6px 4px;
  }
  .name {
    font-size: 17px;
    font-weight: 750;
    letter-spacing: -0.02em;
  }
  .tag {
    font-size: 11.5px;
    color: var(--text-3);
    line-height: 1.25;
  }
  .reader {
    padding: 12px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-sm);
  }
  .reader .label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }
  .select-wrap {
    position: relative;
  }
  .select-wrap :global(.reader-ic) {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-3);
    pointer-events: none;
  }
  .select-wrap select {
    padding-left: 33px;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow: hidden;
    height: 34px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    font-size: 12.5px;
    font-weight: 550;
    color: var(--text-2);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
    position: relative;
  }
  .dot.ok {
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-soft);
  }
  .dot.warn {
    background: var(--warning);
    box-shadow: 0 0 0 3px var(--warning-soft);
  }
  .dot.bad {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .dot.idle {
    background: var(--warning);
    box-shadow: 0 0 0 3px var(--warning-soft);
  }
  .dot.busy {
    background: var(--accent);
  }
  .dot.busy::after,
  .dot.idle::after {
    content: "";
    position: absolute;
    inset: -4px;
    border-radius: 50%;
    border: 2px solid currentColor;
    color: var(--accent);
    animation: ping 1.4s ease-out infinite;
  }
  .dot.idle::after {
    color: var(--warning);
    animation-duration: 2.4s;
  }
  @keyframes ping {
    from {
      transform: scale(0.6);
      opacity: 0.9;
    }
    to {
      transform: scale(1.6);
      opacity: 0;
    }
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    height: 38px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--text-2);
    font-weight: 550;
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .nav-item:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .nav-item.active::before {
    content: "";
    position: absolute;
    left: -14px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
  }
  .spacer {
    flex: 1;
  }
  .demo {
    display: flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
    margin-left: 8px;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--warning-soft);
    color: var(--warning-text);
  }
</style>
