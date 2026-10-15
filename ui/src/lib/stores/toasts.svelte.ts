// Toast notifications.
export type ToastLevel = "info" | "success" | "error";

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface Toast {
  id: number;
  level: ToastLevel;
  title: string;
  detail?: string;
  action?: ToastAction;
}

let nextId = 1;
export const toasts = $state<Toast[]>([]);

export function dismissToast(id: number) {
  const i = toasts.findIndex((x) => x.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export function toast(level: ToastLevel, title: string, opts: { detail?: string; action?: ToastAction; timeout?: number } = {}) {
  const id = nextId++;
  toasts.push({ id, level, title, detail: opts.detail, action: opts.action });
  while (toasts.length > 4) toasts.shift();
  const timeout = opts.timeout ?? (level === "error" ? 8000 : opts.action ? 7000 : 4000);
  if (timeout > 0) setTimeout(() => dismissToast(id), timeout);
  return id;
}
