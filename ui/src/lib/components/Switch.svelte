<script lang="ts">
  let { checked = $bindable(false), label, id }: { checked: boolean; label: string; id?: string } = $props();
</script>

<button
  {id}
  type="button"
  role="switch"
  class="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={() => (checked = !checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    width: 40px;
    height: 23px;
    flex: none;
    border-radius: 999px;
    border: 0;
    background: var(--border-strong);
    cursor: pointer;
    transition: background 0.2s;
  }
  .switch[aria-checked="true"] {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    transition: transform 0.2s var(--ease);
  }
  .switch[aria-checked="true"] .knob {
    transform: translateX(17px);
  }
</style>
