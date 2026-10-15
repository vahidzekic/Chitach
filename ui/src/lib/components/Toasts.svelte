<script lang="ts">
  import { fly } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { toasts, dismissToast } from "../stores/toasts.svelte";
  import { t } from "../i18n/index.svelte";

  const icon: Record<string, IconName> = { info: "info", success: "checkCircle", error: "alertCircle" };
</script>

<div class="toasts" role="region" aria-label={t("ui.notifications")} aria-live="polite">
  {#each toasts as toast (toast.id)}
    <div
      class="toast {toast.level}"
      role={toast.level === "error" ? "alert" : "status"}
      in:fly={{ y: 16, duration: 220 }}
      out:fly={{ x: 40, duration: 180 }}
    >
      <span class="ic"><Icon name={icon[toast.level]} size={18} stroke={2} /></span>
      <div class="body">
        <div class="title">{toast.title}</div>
        {#if toast.detail}<div class="detail">{toast.detail}</div>{/if}
      </div>
      {#if toast.action}
        {@const action = toast.action}
        <button
          class="btn btn-sm action"
          type="button"
          onclick={() => {
            action.run();
            dismissToast(toast.id);
          }}>{action.label}</button
        >
      {/if}
      <button class="close" type="button" aria-label={t("ui.close")} onclick={() => dismissToast(toast.id)}>
        <Icon name="x" size={14} stroke={2} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 76px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 100;
    width: min(380px, calc(100vw - 40px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 10px 12px 14px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    position: relative;
    overflow: hidden;
  }
  .toast::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--accent);
  }
  .ic {
    margin-top: 1px;
    color: var(--accent);
  }
  .success::before {
    background: var(--success);
  }
  .success .ic {
    color: var(--success);
  }
  .error::before {
    background: var(--danger);
  }
  .error .ic {
    color: var(--danger);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 13.5px;
  }
  .detail {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .action {
    align-self: center;
  }
  .close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }
  .close:hover {
    background: var(--surface-3);
    color: var(--text);
  }
</style>
