<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import InsertCard from "../../components/InsertCard.svelte";
  import { api } from "../../api";
  import { t, translateError } from "../../i18n/index.svelte";
  import { app, showError } from "../../stores/app.svelte";
  import { copyText } from "../../util/format";
  import { toast } from "../../stores/toasts.svelte";

  const status = $derived(app.state.status);
  const err = $derived(app.state.error ? translateError(app.state.error) : null);

  async function retry() {
    try {
      await api.reloadCard();
    } catch (e) {
      showError(e);
    }
  }

  async function copyAtr() {
    if (app.state.atr && (await copyText(app.state.atr))) toast("success", t("ui.contentCopied"));
  }
</script>

{#if status === "reading"}
  <div class="reading" aria-busy="true" aria-live="polite">
    <div class="progress"><span></span></div>
    <div class="reading-label">
      <Icon name="refresh" size={15} class="spin" />
      <span>{t("poller.readingFromCard")}</span>
      <span class="faint">· {t("state.reading.desc")}</span>
    </div>
    <div class="sk-hero card">
      <div class="sk sk-portrait"></div>
      <div class="sk-lines">
        <div class="sk" style="width: 120px; height: 22px; border-radius: 99px"></div>
        <div class="sk" style="width: 60%; height: 28px"></div>
        <div class="sk" style="width: 35%"></div>
        <div class="sk-row">
          <div class="sk" style="height: 34px"></div>
          <div class="sk" style="height: 34px"></div>
          <div class="sk" style="height: 34px"></div>
        </div>
      </div>
    </div>
    <div class="sk-grid">
      {#each [6, 4, 5, 3] as rows, i (i)}
        <div class="card sk-card">
          <div class="sk" style="width: 45%; height: 18px; margin-bottom: 16px"></div>
          <div class="sk-fields">
            {#each Array(rows) as _, j (j)}
              <div>
                <div class="sk" style="width: 50%; height: 9px; margin-bottom: 6px"></div>
                <div class="sk" style="width: 80%"></div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
{:else}
  <div class="empty" class:tone-bad={status === "error"}>
    {#if status === "waiting"}
      <InsertCard />
      <h1>{t("state.waiting.title")}</h1>
      <p>{t("state.waiting.desc")}</p>
      <div class="chips">
        <span class="chip"><Icon name="card" size={15} />{t("doc.type.id")}</span>
        <span class="chip"><Icon name="medical" size={15} />{t("doc.type.medical")}</span>
        <span class="chip"><Icon name="car" size={15} />{t("doc.type.vehicle")}</span>
      </div>
    {:else if status === "noReader"}
      <InsertCard variant="noReader" />
      <h1>{t("error.noReader")}</h1>
      <p>{t("error.noReaderExplanation")} {t("state.noReader.hint")}</p>
    {:else if status === "unknownCard"}
      <div class="big-ic warn"><Icon name="alert" size={34} /></div>
      <h1>{t("error.unknownCard")}</h1>
      <p>{t("state.unknown.desc")}</p>
      {#if app.state.atr}
        <div class="atr">
          <div class="atr-label">{t("ui.atr")}</div>
          <code class="mono">{app.state.atr}</code>
          <button class="btn btn-sm" type="button" onclick={copyAtr}>
            <Icon name="copy" size={14} />{t("error.unknownCard.copyAtr")}
          </button>
        </div>
      {/if}
    {:else if status === "error"}
      <div class="big-ic bad"><Icon name="alertCircle" size={34} /></div>
      <h1>{err?.title ?? t("error.readingCard")}</h1>
      {#if err?.detail}<p class="detail mono">{err.detail}</p>{/if}
      <p>{t("state.error.desc")}</p>
      <button class="btn btn-primary" type="button" onclick={retry}>
        <Icon name="refresh" size={16} />{t("ui.retry")}
      </button>
    {/if}
  </div>
{/if}

<style>
  .empty {
    min-height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 32px;
    gap: 10px;
    animation: fade-up 0.4s var(--ease) both;
  }
  .empty h1 {
    margin-top: 14px;
    font-size: 21px;
  }
  .empty p {
    max-width: 460px;
    color: var(--text-2);
  }
  .empty .btn {
    margin-top: 10px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 12px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    font-size: 12.5px;
    font-weight: 550;
  }
  .big-ic {
    width: 76px;
    height: 76px;
    border-radius: 24px;
    display: grid;
    place-items: center;
  }
  .big-ic.warn {
    background: var(--warning-soft);
    color: var(--warning);
  }
  .big-ic.bad {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .detail {
    padding: 6px 10px;
    border-radius: 8px;
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .atr {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 16px 18px;
    border-radius: var(--radius);
    border: 1px dashed var(--border-strong);
    background: var(--surface);
    max-width: 520px;
  }
  .atr-label {
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .atr code {
    font-size: 13px;
    overflow-wrap: anywhere;
    user-select: all;
  }

  /* skeleton */
  .reading {
    padding: 28px 32px;
    max-width: 1080px;
    margin: 0 auto;
  }
  .progress {
    height: 3px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
    margin-bottom: 12px;
  }
  .progress span {
    display: block;
    height: 100%;
    width: 35%;
    border-radius: 3px;
    background: var(--accent);
    animation: indeterminate 1.3s ease-in-out infinite;
  }
  @keyframes indeterminate {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
  .reading-label {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
    font-weight: 600;
    color: var(--accent-text);
  }
  .reading-label .faint {
    font-weight: 450;
  }
  .sk {
    height: 14px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--skeleton) 25%, var(--skeleton-shine) 50%, var(--skeleton) 75%);
    background-size: 300% 100%;
    animation: shimmer 1.4s ease-in-out infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }
  .sk-hero {
    display: flex;
    gap: 24px;
    padding: 22px;
    margin-bottom: 16px;
  }
  .sk-portrait {
    width: 132px;
    height: 167px;
    border-radius: 12px;
    flex: none;
  }
  .sk-lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 12px;
  }
  .sk-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    margin-top: 6px;
  }
  .sk-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 16px;
  }
  .sk-card {
    padding: 18px;
  }
  .sk-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px 14px;
  }
</style>
