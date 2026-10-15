<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { api } from "../api";
  import { t } from "../i18n/index.svelte";
  import { app, checkForUpdate } from "../stores/app.svelte";

  const REPO = "https://github.com/vahidzekic/Chitach";
  let checking = $state(false);

  async function check() {
    checking = true;
    await checkForUpdate(false);
    checking = false;
  }

  const osName = $derived(
    app.info ? ({ linux: "Linux", macos: "macOS", windows: "Windows" } as const)[app.info.os] : "",
  );
</script>

<div class="page">
  <section class="card hero">
    <Logo size={72} />
    <div>
      <h1>Chitach</h1>
      <p class="tag">{t("app.tagline")}</p>
      <div class="meta">
        <span class="badge badge-accent">{t("about.version")} {app.info?.version ?? "—"}</span>
        {#if app.info}<span class="badge">{osName} · {app.info.arch}</span>{/if}
        <span class="badge">{t("about.license")}: MIT</span>
      </div>
    </div>
  </section>

  <div class="grid">
    <section class="card box">
      <h2><Icon name="info" size={16} />{t("about.moreAboutProgram")}</h2>
      <p>{t("about.description")}</p>
      <p class="muted">
        {t("about.author")}: Vahid Zekic ·
        <a href={REPO} onclick={(e) => { e.preventDefault(); void api.openUrl(REPO); }}>github.com/vahidzekic/Chitach</a>
      </p>
      <p class="privacy"><Icon name="lock" size={15} />{t("about.privacy")}</p>
      <div class="links">
        <button class="btn btn-sm" type="button" onclick={() => void api.openUrl(REPO)}>
          <Icon name="code" size={15} />{t("about.sourceCode")}
        </button>
        <button class="btn btn-sm" type="button" onclick={() => void api.openUrl(`${REPO}#readme`)}>
          <Icon name="external" size={15} />{t("about.guide")}
        </button>
      </div>
    </section>

    <section class="card box">
      <h2><Icon name="sparkle" size={16} />{t("about.checkUpdate")}</h2>
      {#if app.update}
        {#if app.update.newer}
          <div class="upd new">
            <Icon name="download" size={18} />
            <div>
              <strong>{t("about.newVersionAvailable", app.update.latest)}</strong>
              <button class="btn btn-primary btn-sm" type="button" onclick={() => void api.openUrl(app.update!.url)}>
                {t("ui.download")}
              </button>
            </div>
          </div>
        {:else}
          <div class="upd ok"><Icon name="checkCircle" size={18} /><strong>{t("about.youHaveLatestVersion")}</strong></div>
        {/if}
      {/if}
      <button class="btn" type="button" onclick={check} disabled={checking}>
        <Icon name="refresh" size={16} class={checking ? "spin" : ""} />
        {checking ? t("about.checking") : t("about.checkUpdate")}
      </button>
    </section>
  </div>
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 26px;
    margin-bottom: 16px;
    background: var(--hero-grad);
  }
  h1 {
    font-size: 28px;
    letter-spacing: -0.03em;
  }
  .tag {
    color: var(--text-2);
    margin-top: 2px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .meta .badge:not(.badge-accent) {
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 16px;
    align-items: start;
  }
  .box {
    padding: 18px 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
  }
  .box h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14.5px;
  }
  .box h2 :global(svg) {
    color: var(--accent);
  }
  .box p {
    font-size: 13.5px;
  }
  .privacy {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--success-soft);
    color: var(--success-text);
    font-weight: 550;
    font-size: 12.5px !important;
  }
  .links {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .upd {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    width: 100%;
    padding: 12px;
    border-radius: var(--radius-sm);
  }
  .upd div {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .upd.new {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .upd.ok {
    background: var(--success-soft);
    color: var(--success-text);
  }
</style>
