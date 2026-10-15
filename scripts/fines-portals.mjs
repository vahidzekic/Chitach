#!/usr/bin/env node
// Weekly health check of the fines portals (.github/workflows/fines-portals.yml).
//
// For every provider in src-tauri/providers/fines.json:
//   - the page (`url`) must answer (2xx/3xx). 401/403/429 count as "protected" (bot
//     protection, a warning, not a failure);
//   - an automatic check (`json_get`, `html_form`, `json_post`) is sent once with a test plate and
//     its answer must have the shape the app's parser expects;
//   - the page is summarised (title, forms with their fields, iframes, captcha) so link-only
//     cities can be turned into automatic checks, and the bodies are saved as samples.
//
//   node scripts/fines-portals.mjs [--out dir] [--defs path] [--only id,id]
// Exit code 1 if anything failed. Node 22+, no dependencies.
import { mkdirSync, readFileSync, writeFileSync, appendFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : def;
};
const defsPath = opt("--defs", join(root, "src-tauri", "providers", "fines.json"));
const outDir = opt("--out", join(root, "target", "fines-portals"));
const only = opt("--only", "")?.split(",").filter(Boolean);
const version = JSON.parse(readFileSync(join(root, "ui", "package.json"), "utf8")).version;
const USER_AGENT = `Chitach/${version} portal-check (+https://github.com/vahidzekic/Chitach)`;
const TIMEOUT_MS = 20_000;

const CYR = {
  А: "A", Б: "B", В: "V", Г: "G", Д: "D", Ђ: "Đ", Е: "E", Ж: "Ž", З: "Z", И: "I", Ј: "J", К: "K", Л: "L", Љ: "LJ",
  М: "M", Н: "N", Њ: "NJ", О: "O", П: "P", Р: "R", С: "S", Т: "T", Ћ: "Ć", У: "U", Ф: "F", Х: "H", Ц: "C", Ч: "Č",
  Џ: "DŽ", Ш: "Š",
};
// Same rules as fines.rs `normalize_text` / `html_text` (simplified).
const normText = (s) => [...s.toUpperCase()].map((c) => CYR[c] ?? c).join("").toLowerCase().replace(/\s+/g, " ").trim();
const htmlText = (h) =>
  h
    .replace(/<!--[\s\S]*?-->/g, " ")
    .replace(/<(script|style)\b[\s\S]*?<\/\1\s*>/gi, " ")
    .replace(/<[^>]*>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)));
const CAPTCHA = ["g-recaptcha", "cf-turnstile", "h-captcha", "challenge-platform", "captcha"];

const expand = (p, t) =>
  t.replaceAll("{route}", encodeURIComponent(p.route ?? "")).replaceAll("{city}", encodeURIComponent(p.cities?.[0] ?? ""));
// A plate that is very unlikely to have tickets; only sent to portals the app itself uses.
const testPlate = (p) => `${p.cities?.[0] ?? "BG"}999ZZ`;

async function request(method, url, form, json) {
  const headers = { "User-Agent": USER_AGENT, Accept: "text/html,application/json;q=0.9,*/*;q=0.5" };
  let body;
  if (json) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(json);
  } else if (form && method === "POST") {
    headers["Content-Type"] = "application/x-www-form-urlencoded";
    body = new URLSearchParams(form).toString();
  } else if (form && Object.keys(form).length) {
    url += (url.includes("?") ? "&" : "?") + new URLSearchParams(form).toString();
  }
  const started = Date.now();
  try {
    const rsp = await fetch(url, { method, headers, body, redirect: "follow", signal: AbortSignal.timeout(TIMEOUT_MS) });
    const text = await rsp.text();
    return { status: rsp.status, contentType: rsp.headers.get("content-type") ?? "", finalUrl: rsp.url, text, ms: Date.now() - started };
  } catch (e) {
    return { error: String(e?.cause?.code ?? e?.name ?? e), ms: Date.now() - started };
  }
}

function summarize(html) {
  const lower = html.toLowerCase();
  const title = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(html)?.[1]?.trim().replace(/\s+/g, " ") ?? "";
  const forms = [...html.matchAll(/<form\b([^>]*)>([\s\S]*?)<\/form>/gi)].map((m) => {
    const attr = (a, n) => new RegExp(`${n}\\s*=\\s*["']?([^"'\\s>]*)`, "i").exec(a)?.[1] ?? "";
    const fields = [...m[2].matchAll(/<(input|select|textarea)\b([^>]*)>/gi)]
      .map((f) => `${attr(f[2], "name") || "?"}${attr(f[2], "type") ? `:${attr(f[2], "type")}` : ""}`);
    return `${(attr(m[1], "method") || "get").toUpperCase()} ${attr(m[1], "action") || "(same page)"} [${fields.join(", ")}]`;
  });
  const iframes = [...html.matchAll(/<iframe\b[^>]*src\s*=\s*["']([^"']+)/gi)].map((m) => m[1]);
  const captcha = CAPTCHA.filter((c) => lower.includes(c));
  return { title, forms, iframes, captcha };
}

function checkShape(p, r) {
  const c = p.check;
  if (p.kind === "json_get" || p.kind === "json_post") {
    // Same rules as fines.rs `parse_json_array` / `interpret`.
    if (c.emptyIsClean && ["", '""'].includes(r.text.trim())) return null;
    if (/html/i.test(r.contentType) || r.text.trimStart().startsWith("<")) return "HTML instead of JSON";
    let v;
    try {
      v = JSON.parse(r.text);
      if (typeof v === "string") v = JSON.parse(v);
    } catch {
      return "not JSON";
    }
    const arrays = Array.isArray(v) ? [v] : v && typeof v === "object" ? Object.values(v).filter(Array.isArray) : [];
    if (arrays.length !== 1) return "JSON without a single array";
    const rows = arrays[0];
    const errorRow = (r) => typeof r?.response === "string" && r.response.toUpperCase().includes("ERROR");
    return rows.length && rows.every(errorRow) ? "error row instead of tickets" : null;
  }
  if (p.kind === "html_form") {
    const lower = r.text.toLowerCase();
    if (CAPTCHA.some((m) => lower.includes(m))) return "captcha";
    const text = normText(htmlText(r.text));
    const has = (ms) => (ms ?? []).some((m) => normText(m) && text.includes(normText(m)));
    return has(c.found) || has(c.clean) ? null : "no found/clean marker on the answer page";
  }
  return null;
}

const defs = JSON.parse(readFileSync(defsPath, "utf8"));
mkdirSync(outDir, { recursive: true });
const rows = [];
const details = [];
let failures = 0;

for (const p of defs.providers) {
  if (only?.length && !only.includes(p.id)) continue;
  const plate = testPlate(p);
  const pageUrl = expand(p, p.url).replaceAll("{plate}", encodeURIComponent(plate));
  const page = await request("GET", pageUrl);
  let pageState;
  if (page.error) pageState = `FAIL ${page.error}`;
  else if ([401, 403, 429].includes(page.status)) pageState = `protected (HTTP ${page.status})`;
  else if (page.status >= 400) pageState = `FAIL HTTP ${page.status}`;
  else pageState = `OK ${page.status}`;
  if (pageState.startsWith("FAIL")) failures++;
  if (page.text !== undefined) writeFileSync(join(outDir, `${p.id}.page.html`), page.text);

  let checkState = "—";
  if (p.check && ["json_get", "html_form", "json_post"].includes(p.kind)) {
    const c = p.check;
    const method = p.kind === "json_post" ? "POST" : (c.method ?? "get").toUpperCase();
    const json =
      p.kind === "json_post"
        ? Object.fromEntries(
            Object.entries(c.json ?? {}).map(([k, v]) => [
              k,
              typeof v === "string" ? v.replaceAll("{route}", p.route ?? "").replaceAll("{city}", p.cities?.[0] ?? "").replaceAll("{plate}", plate) : v,
            ]),
          )
        : undefined;
    const form = Object.fromEntries(Object.entries(c.form ?? {}).map(([k, v]) => [k, expand(p, v).replaceAll("{plate}", plate)]));
    const url = expand(p, c.url).replaceAll("{plate}", encodeURIComponent(plate));
    const r = await request(method, url, form, json);
    if (r.error) checkState = `FAIL ${r.error}`;
    else if ([401, 403, 429].includes(r.status)) checkState = `FAIL protected (HTTP ${r.status}): the app falls back to "check by hand"`;
    else if (r.status >= 400) checkState = `FAIL HTTP ${r.status}`;
    else {
      const problem = checkShape(p, r);
      checkState = problem ? `FAIL ${problem}` : `OK ${r.status}, ${r.ms} ms`;
    }
    if (checkState.startsWith("FAIL")) failures++;
    if (r.text !== undefined) writeFileSync(join(outDir, `${p.id}.check.txt`), r.text);
  }

  rows.push(`| \`${p.id}\` | ${p.kind} | ${pageState} | ${checkState} |`);
  if (page.text !== undefined) {
    const s = summarize(page.text);
    details.push(
      [
        `### ${p.id} — ${pageUrl}`,
        page.finalUrl && page.finalUrl !== pageUrl ? `- redirected to ${page.finalUrl}` : null,
        s.title ? `- title: ${s.title}` : null,
        ...s.forms.map((f) => `- form: \`${f}\``),
        ...s.iframes.map((f) => `- iframe: ${f}`),
        s.captcha.length ? `- captcha markers: ${s.captcha.join(", ")}` : null,
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }
}

const report = [
  `## Fines portals — ${new Date().toISOString().slice(0, 10)} (definitions ${defs.updated})`,
  "",
  "| Provider | Kind | Page | Automatic check |",
  "|---|---|---|---|",
  ...rows,
  "",
  failures ? `**${failures} problem(s).**` : "All portals answered as expected.",
  "",
  "<details><summary>Page structure (forms, iframes, captcha)</summary>",
  "",
  ...details.flatMap((d) => [d, ""]),
  "</details>",
  "",
].join("\n");

writeFileSync(join(outDir, "report.md"), report);
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, report);
console.log(report);
process.exit(failures ? 1 : 0);
