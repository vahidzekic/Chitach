<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "../i18n/index.svelte";
  import { copyText } from "../util/format";

  let {
    label,
    value,
    wide = false,
    mono = false,
    suffix = "",
  }: { label: string; value: string | null | undefined; wide?: boolean; mono?: boolean; suffix?: string } = $props();

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const empty = $derived(!value || !value.trim());

  async function copy() {
    if (empty || !value) return;
    if (await copyText(value)) {
      copied = true;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1600);
    }
  }
</script>

<div class="field" class:wide class:empty>
  <div class="lbl">{label}</div>
  <div class="val" class:mono>
    {#if empty}
      <span class="none">—</span>
    {:else}
      {value}{#if suffix}<span class="suffix">&nbsp;{suffix}</span>{/if}
    {/if}
  </div>
  {#if !empty}
    <button
      class="copy"
      class:copied
      type="button"
      onclick={copy}
      aria-label={t("ui.copyField", label)}
      title={copied ? t("ui.contentCopied") : t("ui.copy")}
    >
      <Icon name={copied ? "check" : "copy"} size={14} stroke={2} />
    </button>
    {#if copied}<span class="tip" role="status">{t("ui.contentCopied")}</span>{/if}
  {/if}
</div>

<style>
  .field {
    position: relative;
    padding: 8px 34px 8px 10px;
    margin: 0 -10px;
    border-radius: var(--radius-sm);
    min-width: 0;
    transition: background 0.15s;
  }
  .field.wide {
    grid-column: 1 / -1;
  }
  .field:not(.empty):hover,
  .field:focus-within {
    background: var(--surface-2);
  }
  .lbl {
    font-size: 11.5px;
    font-weight: 550;
    color: var(--text-3);
    letter-spacing: 0.01em;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .val {
    font-size: 14px;
    font-weight: 520;
    color: var(--text);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .val.mono {
    font-family: var(--mono);
    font-size: 13.5px;
    letter-spacing: 0.02em;
  }
  .suffix {
    color: var(--text-3);
    font-weight: 450;
    font-family: var(--font);
  }
  .none {
    color: var(--text-3);
  }
  .copy {
    position: absolute;
    top: 50%;
    right: 6px;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity 0.15s,
      color 0.15s;
  }
  .field:hover .copy,
  .copy:focus-visible,
  .copy.copied {
    opacity: 1;
  }
  .copy:hover {
    color: var(--accent-text);
  }
  .copy.copied {
    color: var(--success);
    border-color: var(--success);
  }
  .tip {
    position: absolute;
    right: 4px;
    top: -18px;
    padding: 2px 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    background: var(--text);
    color: var(--surface);
    pointer-events: none;
    animation: fade-up 0.2s var(--ease) both;
    z-index: 2;
  }
</style>
