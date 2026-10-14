<script lang="ts">
  import Section from "../../components/Section.svelte";
  import Field from "../../components/Field.svelte";
  import { visible, type SectionDef } from "./fields";

  let { sections }: { sections: SectionDef[] } = $props();
</script>

<div class="sections">
  {#each sections as s, i (i)}
    {@const fields = visible(s.fields)}
    {#if fields.length > 0}
      <Section title={s.title} icon={s.icon} note={s.note}>
        {#each fields as f, j (j)}
          <Field label={f.label} value={f.value} wide={f.wide} mono={f.mono} suffix={f.suffix} />
        {/each}
      </Section>
    {/if}
  {/each}
</div>

<style>
  .sections {
    columns: 2 340px;
    column-gap: 16px;
  }
  .sections > :global(*) {
    break-inside: avoid;
    margin-bottom: 16px;
  }
</style>
