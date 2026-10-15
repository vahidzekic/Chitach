<script lang="ts" generics="T extends string">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    value = $bindable(),
    options,
    label,
  }: { value: T; options: { value: T; label: string; icon?: IconName }[]; label: string } = $props();

  function key(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    let n = i;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") n = (i + 1) % options.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") n = (i - 1 + options.length) % options.length;
    else return;
    e.preventDefault();
    value = options[n].value;
    const el = (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>("button")[n];
    el?.focus();
  }
</script>

<div class="seg" role="radiogroup" aria-label={label} tabindex="-1" onkeydown={key}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      tabindex={value === o.value ? 0 : -1}
      onclick={() => (value = o.value)}
    >
      {#if o.icon}<Icon name={o.icon} size={15} />{/if}
      <span>{o.label}</span>
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface-3);
    border: 1px solid var(--border);
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    font-weight: 550;
    cursor: pointer;
    transition:
      background 0.15s,
      color 0.15s;
  }
  button:hover {
    color: var(--text);
  }
  button[aria-checked="true"] {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-sm), 0 0 0 1px var(--border);
  }
</style>
