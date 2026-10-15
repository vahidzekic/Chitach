#!/usr/bin/env node
// Weekly check that the pages in the embedded provider definitions still answer
// (src-tauri/providers/fines.json, history.json). Only the user-facing pages are requested,
// with sample values for the placeholders; automatic-check endpoints are never called.
//
// Exit code 1 when a page is gone (404/410, unknown domain, other errors). 401/403/429/503
// and timeouts count as warnings: many portals block or slow down automated clients
// (Cloudflare, captchas, geo-blocking).
import { readFileSync, appendFileSync } from "node:fs";

const FILES = ["src-tauri/providers/fines.json", "src-tauri/providers/history.json"];
const SAMPLE = { plate: "BG123AB", vin: "TMBJJ7NE0L0123456" };
const WARN = new Set([401, 403, 405, 406, 429, 503]);
const WARN_ERRORS = new Set(["TimeoutError", "UND_ERR_CONNECT_TIMEOUT", "ECONNRESET", "ETIMEDOUT"]);
const UA = "Chitach link check (+https://github.com/vahidzekic/Chitach)";

function resolve(url, p) {
  return url
    .replaceAll("{plate}", SAMPLE.plate)
    .replaceAll("{vin}", SAMPLE.vin)
    .replaceAll("{route}", encodeURIComponent(p.route ?? ""))
    .replaceAll("{city}", encodeURIComponent(p.cities?.[0] ?? ""));
}

async function probe(url) {
  for (const method of ["HEAD", "GET"]) {
    try {
      const r = await fetch(url, {
        method,
        redirect: "follow",
        headers: { "User-Agent": UA },
        signal: AbortSignal.timeout(20_000),
      });
      if (method === "HEAD" && (r.status === 405 || r.status === 403 || r.status >= 500)) continue;
      return { status: r.status };
    } catch (e) {
      if (method === "GET") return { error: e.cause?.code ?? e.name ?? e.message };
    }
  }
  return { error: "no response" };
}

const rows = [];
for (const file of FILES) {
  const { providers } = JSON.parse(readFileSync(file, "utf8"));
  for (const p of providers) {
    for (const key of ["url", "altUrl"]) {
      if (!p[key]) continue;
      const url = resolve(p[key], p);
      const r = await probe(url);
      const level = r.error
        ? WARN_ERRORS.has(r.error)
          ? "warn"
          : "fail"
        : r.status < 400
          ? "ok"
          : WARN.has(r.status)
            ? "warn"
            : "fail";
      rows.push({ file: file.split("/").pop(), id: p.id, url, level, result: r.error ?? `HTTP ${r.status}` });
      console.log(`${level.padEnd(4)} ${p.id.padEnd(22)} ${r.error ?? r.status}  ${url}`);
    }
  }
}

const failed = rows.filter((r) => r.level === "fail");
if (process.env.GITHUB_STEP_SUMMARY) {
  const icon = { ok: "✅", warn: "⚠️", fail: "❌" };
  const md = [
    "## Provider pages",
    "",
    "| | file | id | result | url |",
    "|---|---|---|---|---|",
    ...rows.map((r) => `| ${icon[r.level]} | ${r.file} | ${r.id} | ${r.result} | ${r.url} |`),
    "",
  ].join("\n");
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, md);
}
if (failed.length) {
  console.error(`\n${failed.length} page(s) failed: ${failed.map((r) => r.id).join(", ")}`);
  process.exit(1);
}
