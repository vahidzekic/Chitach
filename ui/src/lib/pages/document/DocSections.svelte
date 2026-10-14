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
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 16px;
    align-items: start;
  }
  @media (max-width: 760px) {
    .sections {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
