<script lang="ts">
  import Modal from "../components/Modal.svelte";
  import Icon from "../components/Icon.svelte";
  import { api } from "../api";
  import { t, translateError } from "../i18n/index.svelte";
  import { toast } from "../stores/toasts.svelte";

  let {
    open = $bindable(false),
    triesLeft = null,
    onchanged,
  }: { open: boolean; triesLeft?: number | null; onchanged?: () => void } = $props();

  let oldPin = $state("");
  let newPin = $state("");
  let confirmPin = $state("");
  let show = $state(false);
  let busy = $state(false);
  let submitted = $state(false);
  let serverError = $state<string | null>(null);

  // Port of card.ValidatePin (bas-celik/card/pin.go): 4-8 digits.
  const validatePin = (pin: string) => /^[0-9]{4,8}$/.test(pin);

  const errors = $derived({
    old: oldPin && !validatePin(oldPin) ? t("pinChange.oldPinFormatError") : null,
    new: newPin && !validatePin(newPin) ? t("pinChange.newPinFormatError") : null,
    confirm:
      confirmPin && !validatePin(confirmPin)
        ? t("pinChange.confirmNewPinFormatError")
        : confirmPin && newPin && confirmPin !== newPin
          ? t("pinChange.pinsNotEqual")
          : null,
  });
  const valid = $derived(
    validatePin(oldPin) && validatePin(newPin) && validatePin(confirmPin) && newPin === confirmPin,
  );

  $effect(() => {
    if (open) {
      oldPin = newPin = confirmPin = "";
      submitted = false;
      serverError = null;
      show = false;
    }
  });

  function digitsOnly(e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const v = el.value.replace(/\D/g, "").slice(0, 8);
    if (v !== el.value) el.value = v;
    return v;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    submitted = true;
    if (!valid || busy) return;
    busy = true;
    serverError = null;
    try {
      await api.changePin(oldPin, newPin);
      toast("success", t("pinChange.success"));
      open = false;
      onchanged?.();
    } catch (err) {
      const { title, detail } = translateError(err);
      serverError = detail ? `${title} (${detail})` : title;
      onchanged?.();
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title={t("pinChange.title")}>
  <form id="pin-form" onsubmit={submit} novalidate>
    <p class="hint">
      <Icon name="info" size={15} />
      <span>
        {t("pinChange.pinFormatExplanation")}
        {#if triesLeft !== null}<strong>{t("pinChange.triesLeft")}: {triesLeft}</strong>{/if}
      </span>
    </p>

    <div class="row">
      <label class="label" for="pin-old">{t("pinChange.oldPin")}</label>
      <input
        id="pin-old"
        class="input mono"
        type={show ? "text" : "password"}
        inputmode="numeric"
        autocomplete="off"
        maxlength="8"
        value={oldPin}
        oninput={(e) => (oldPin = digitsOnly(e))}
        aria-invalid={!!errors.old || (submitted && !validatePin(oldPin))}
        aria-describedby="pin-old-err"
      />
      <div class="err" id="pin-old-err">{errors.old ?? ""}</div>
    </div>
    <div class="row">
      <label class="label" for="pin-new">{t("pinChange.newPin")}</label>
      <input
        id="pin-new"
        class="input mono"
        type={show ? "text" : "password"}
        inputmode="numeric"
        autocomplete="off"
        maxlength="8"
        value={newPin}
        oninput={(e) => (newPin = digitsOnly(e))}
        aria-invalid={!!errors.new || (submitted && !validatePin(newPin))}
        aria-describedby="pin-new-err"
      />
      <div class="err" id="pin-new-err">{errors.new ?? ""}</div>
    </div>
    <div class="row">
      <label class="label" for="pin-confirm">{t("pinChange.confirmNewPin")}</label>
      <input
        id="pin-confirm"
        class="input mono"
        type={show ? "text" : "password"}
        inputmode="numeric"
        autocomplete="off"
        maxlength="8"
        value={confirmPin}
        oninput={(e) => (confirmPin = digitsOnly(e))}
        aria-invalid={!!errors.confirm || (submitted && !validatePin(confirmPin))}
        aria-describedby="pin-confirm-err"
      />
      <div class="err" id="pin-confirm-err">{errors.confirm ?? ""}</div>
    </div>

    <label class="show">
      <input type="checkbox" bind:checked={show} />
      {t("ui.showPin")}
    </label>

    {#if serverError}
      <div class="server-error" role="alert"><Icon name="alertCircle" size={16} />{serverError}</div>
    {/if}
  </form>

  {#snippet actions()}
    <button class="btn" type="button" onclick={() => (open = false)}>{t("pinChange.cancel")}</button>
    <button class="btn btn-primary" type="submit" form="pin-form" disabled={busy || !valid}>
      {#if busy}<Icon name="refresh" size={15} class="spin" />{/if}
      {t("pinChange.change")}
    </button>
  {/snippet}
</Modal>

<style>
  .hint {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 12.5px;
    margin-bottom: 14px;
  }
  .hint :global(svg) {
    margin-top: 1px;
  }
  .hint strong {
    display: block;
    margin-top: 2px;
  }
  .row {
    margin-bottom: 4px;
  }
  .input {
    letter-spacing: 0.25em;
  }
  .err {
    min-height: 18px;
    padding-top: 3px;
    font-size: 12px;
    color: var(--danger-text);
  }
  .show {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
    cursor: pointer;
  }
  .show input {
    accent-color: var(--accent);
  }
  .server-error {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger-text);
    font-size: 13px;
    font-weight: 550;
  }
</style>
