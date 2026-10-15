import type { Theme } from "../api/types";

const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: Theme = "system";

function resolve(): "light" | "dark" {
  if (current === "system") return media.matches ? "dark" : "light";
  return current;
}

function apply() {
  document.documentElement.dataset.theme = resolve();
}

media.addEventListener("change", () => {
  if (current === "system") apply();
});

export function setTheme(theme: Theme) {
  current = theme;
  apply();
}
