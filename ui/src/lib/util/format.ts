/** Parses dates like "01.02.2030." or "01.02.2030" (as produced by the backend). */
export function parseDate(s: string | null | undefined): Date | null {
  if (!s) return null;
  const m = /^\s*(\d{1,2})\.(\d{1,2})\.(\d{4})\.?\s*$/.exec(s);
  if (!m) return null;
  const d = new Date(Number(m[3]), Number(m[2]) - 1, Number(m[1]), 23, 59, 59);
  return isNaN(d.getTime()) ? null : d;
}

export function isPast(s: string): boolean | null {
  const d = parseDate(s);
  return d ? d.getTime() < Date.now() : null;
}

export function joinComma(...parts: (string | undefined | null)[]): string {
  return parts.filter((p) => p && p.trim()).join(", ");
}

export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i > 0 ? path.substring(0, i) : "";
}

export function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i >= 0 ? path.substring(i + 1) : path;
}

export function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      return ok;
    } catch {
      return false;
    }
  }
}

/** Certificate dates come as "YYYY-MM-DD HH:MM:SS" (UTC); show them as "DD.MM.YYYY." */
export function formatCertDate(s: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(s);
  return m ? `${m[3]}.${m[2]}.${m[1]}.` : s;
}
