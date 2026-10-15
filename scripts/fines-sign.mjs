#!/usr/bin/env node
// Signs the fines provider definitions (src-tauri/providers/fines.json) so the app can
// download newer ones without a new release (see src-tauri/src/fines_defs.rs and
// docs/07-istrazivanje-provera-kazni.md, section 12). Ed25519, Node built-ins only.
//
//   node scripts/fines-sign.mjs keygen [key.pem]   new key pair: private key to key.pem
//                                                  (default fines-signing-key.pem, never
//                                                  commit it), public key to fines.pub
//   node scripts/fines-sign.mjs sign [key.pem]     writes fines.json.sig; the key comes from
//                                                  the file or the FINES_SIGNING_KEY env var
//   node scripts/fines-sign.mjs verify             checks fines.json.sig against fines.pub
import { createPrivateKey, createPublicKey, generateKeyPairSync, sign, verify } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "src-tauri", "providers");
const JSON_PATH = join(dir, "fines.json");
const SIG_PATH = join(dir, "fines.json.sig");
const PUB_PATH = join(dir, "fines.pub");

const b64url = (s) => s.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const b64 = (s) => s.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (s.length % 4)) % 4);

function die(msg) {
  console.error(`fines-sign: ${msg}`);
  process.exit(1);
}

function readPublicKey() {
  const text = readFileSync(PUB_PATH, "utf8");
  const key = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"))
    .join("");
  if (!key) die(`${PUB_PATH} has no key yet (run: node scripts/fines-sign.mjs keygen)`);
  if (Buffer.from(key, "base64").length !== 32) die(`${PUB_PATH}: not a base64 Ed25519 key`);
  return createPublicKey({ key: { kty: "OKP", crv: "Ed25519", x: b64url(key) }, format: "jwk" });
}

function rawPublicKey(publicKey) {
  return b64(publicKey.export({ format: "jwk" }).x);
}

function checkSignature() {
  const data = readFileSync(JSON_PATH);
  if (!existsSync(SIG_PATH)) return false;
  const sig = Buffer.from(readFileSync(SIG_PATH, "utf8").trim(), "base64");
  return verify(null, data, readPublicKey(), sig);
}

const [cmd, arg] = process.argv.slice(2);
switch (cmd) {
  case "keygen": {
    const out = arg ?? "fines-signing-key.pem";
    if (existsSync(out)) die(`${out} already exists`);
    const { publicKey, privateKey } = generateKeyPairSync("ed25519");
    writeFileSync(out, privateKey.export({ format: "pem", type: "pkcs8" }), { mode: 0o600 });
    const comments = readFileSync(PUB_PATH, "utf8")
      .split(/\r?\n/)
      .filter((l) => l.startsWith("#"));
    writeFileSync(PUB_PATH, [...comments, rawPublicKey(publicKey), ""].join("\n"));
    console.log(`Private key: ${out}  (keep it secret; add its content as the GitHub secret FINES_SIGNING_KEY)`);
    console.log(`Public key written to ${PUB_PATH} (commit it; it is built into the app)`);
    break;
  }
  case "sign": {
    const pem = arg ? readFileSync(arg, "utf8") : process.env.FINES_SIGNING_KEY;
    if (!pem) die("no key: pass a key file or set FINES_SIGNING_KEY");
    const privateKey = createPrivateKey(pem);
    if (rawPublicKey(createPublicKey(privateKey)) !== rawPublicKey(readPublicKey()))
      die("the signing key does not match fines.pub");
    const sig = sign(null, readFileSync(JSON_PATH), privateKey).toString("base64");
    writeFileSync(SIG_PATH, sig + "\n");
    if (!checkSignature()) die("signature check failed");
    console.log(`Signed ${JSON_PATH}`);
    break;
  }
  case "verify": {
    if (!checkSignature()) die("fines.json.sig is missing or does not match fines.json");
    console.log("fines.json.sig OK");
    break;
  }
  default:
    die("usage: fines-sign.mjs keygen [key.pem] | sign [key.pem] | verify");
}
