// Chitach Pro licence in the UI: load, activate, deactivate (kept in `app.license`).
import { api } from "./api";
import { t } from "../i18n/index.svelte";
import { app, showError } from "../stores/app.svelte";
import { toast } from "../stores/toasts.svelte";

export async function refreshLicense() {
  try {
    app.license = await api.licenseInfo();
  } catch {
    /* not critical: Pro features stay locked */
  }
}

/** Activate a Chitach Pro key; returns true on success. */
export async function activateLicense(key: string): Promise<boolean> {
  try {
    app.license = await api.licenseActivate(key);
    toast("success", t("pro.activated"));
    return true;
  } catch (e) {
    showError(e, "error.license");
    return false;
  }
}

export async function deactivateLicense(): Promise<void> {
  try {
    app.license = await api.licenseDeactivate();
    toast("success", t("pro.deactivated"));
  } catch (e) {
    showError(e, "error.license");
  }
}
