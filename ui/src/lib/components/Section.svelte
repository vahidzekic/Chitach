<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let { title, icon, children, note }: { title: string; icon?: IconName; children: Snippet; note?: string } = $props();
</script>

<section class="section card">
  <header>
    {#if icon}<span class="ic"><Icon name={icon} size={16} /></span>{/if}
    <h2>{title}</h2>
  </header>
  <div class="grid">
    {@render children()}
  </div>
  {#if note}
    <footer>{note}</footer>
  {/if}
</section>

<style>
  .section {
    padding: 16px 18px 12px;
    min-width: 0;
    animation: fade-up 0.35s var(--ease) both;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  h2 {
    font-size: 14.5px;
    font-weight: 650;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: 14px;
  }
  footer {
    margin-top: 6px;
    padding-top: 10px;
    border-top: 1px dashed var(--border);
    font-size: 12.5px;
    color: var(--text-3);
  }
</style>
