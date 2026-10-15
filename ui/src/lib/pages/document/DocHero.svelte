<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import Field from "../../components/Field.svelte";
  import type { BadgeDef, FieldDef } from "./fields";

  let {
    title,
    subtitle,
    badges,
    facts,
    visual,
  }: { title: string; subtitle?: string; badges: BadgeDef[]; facts: FieldDef[]; visual: Snippet } = $props();
</script>

<header class="hero card">
  <div class="visual">{@render visual()}</div>
  <div class="info">
    <div class="badges">
      {#each badges as b, i (i)}
        <span class="badge badge-{b.tone}">
          {#if b.icon}<Icon name={b.icon} size={13} stroke={2.2} />{/if}
          {b.label}
        </span>
      {/each}
    </div>
    <h1 class="title" title={title}>{title}</h1>
    {#if subtitle}<p class="subtitle">{subtitle}</p>{/if}
    <div class="facts">
      {#each facts as f, i (i)}
        <Field label={f.label} value={f.value} mono={f.mono} suffix={f.suffix} />
      {/each}
    </div>
  </div>
</header>

<style>
  .hero {
    display: flex;
    gap: 24px;
    padding: 22px;
    margin-bottom: 16px;
    background: var(--hero-grad);
    position: relative;
    overflow: hidden;
    animation: fade-up 0.3s var(--ease) both;
  }
  .hero::after {
    content: "";
    position: absolute;
    right: -60px;
    top: -80px;
    width: 260px;
    height: 260px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--accent-soft), transparent 70%);
    pointer-events: none;
  }
  .visual {
    flex: none;
    display: flex;
    align-items: center;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    position: relative;
    z-index: 1;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .badge-neutral {
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .title {
    font-size: 26px;
    font-weight: 720;
    letter-spacing: -0.02em;
    line-height: 1.15;
    overflow-wrap: anywhere;
  }
  .subtitle {
    margin-top: 4px;
    color: var(--text-2);
    font-size: 14px;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    column-gap: 14px;
    margin-top: 14px;
    padding: 4px 12px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
    border: 1px solid var(--border);
  }
  @media (max-width: 760px) {
    .hero {
      gap: 18px;
      padding: 18px;
    }
    .title {
      font-size: 22px;
    }
  }
</style>
