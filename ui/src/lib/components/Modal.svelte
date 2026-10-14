<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n/index.svelte";

  let {
    open = $bindable(false),
    title,
    children,
    actions,
    onclose,
  }: { open: boolean; title: string; children: Snippet; actions?: Snippet; onclose?: () => void } = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  function closed() {
    open = false;
    onclose?.();
  }
</script>

<dialog bind:this={dialog} onclose={closed} aria-labelledby="modal-title">
  {#if open}
    <div class="head">
      <h2 id="modal-title">{title}</h2>
      <button class="btn btn-ghost btn-sm btn-icon" type="button" aria-label={t("ui.close")} onclick={() => (open = false)}>
        <Icon name="x" size={16} />
      </button>
    </div>
    <div class="content">{@render children()}</div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  {/if}
</dialog>

<style>
  dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }
  dialog[open] {
    animation: pop 0.2s var(--ease);
  }
  dialog::backdrop {
    background: rgba(8, 12, 22, 0.45);
    backdrop-filter: blur(2px);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.98);
    }
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 18px 4px 22px;
  }
  h2 {
    font-size: 17px;
  }
  .content {
    padding: 10px 22px 18px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 22px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  }
</style>
