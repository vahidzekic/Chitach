# Plan implementacije (Rust + Tauri v2)

Stek: **Rust (Cargo workspace) + Tauri v2 + Svelte 5 / TypeScript**. Arhitektura je opisana u
[02-arhitektura.md](02-arhitektura.md), a ugovor između backenda i UI-ja u
[04-api-ugovor.md](04-api-ugovor.md) (izvor istine za komande i događaje).

## Paralelni tokovi

Posle skeleta (workspace, stub API-ji sa `todo!()`, API ugovor) poslovi se rade paralelno,
svaki tok u svom direktorijumu:

| Tok | Direktorijum | Zadaci | Zavisi od |
|---|---|---|---|
| **T1 – Jezgro kartica** | `crates/celik-core` | PC/SC, APDU, TLV/BER, ATR, Gemalto/Apollo lična karta, zdravstvena, saobraćajna, `Document` model, lokalizacija, PIN, RFZO, primer dokumenti, testovi sa golden podacima iz Baš Čelika | – |
| **T2 – Izvoz** | `crates/celik-export` | PDF identičan Baš Čeliku (Liberation Sans, latinične/ćirilične labele), JSON, XLSX | `Document` API |
| **T3 – Potpisivanje** | `crates/celik-sign` | PKCS#11 (cryptoki), XML-DSig, Smartbox websocket server | – |
| **T4 – Tauri backend** | `src-tauri/src` | Komande, `app-state` događaji, poller nit, podešavanja, tray, auto-čuvanje, provera verzije, mock režim | stub API-ji |
| **T5 – Frontend** | `ui/` | Layout, početni ekran, 3 prikaza dokumenata, podešavanja, i18n (sr-Latn/sr-Cyrl/en/ru), tamna/svetla tema, mock API za browser, screenshot-ovi | API ugovor |
| **T6 – CLI** | `crates/chitach-cli` | `chitach-cli` (lista čitača, ATR, PDF/JSON/XLSX, RFZO) | T1, T2 |
| **T7 – Pakovanje, CI/CD, dokumentacija** | `.github/`, `src-tauri/tauri.conf.json`, `src-tauri/icons/`, `scripts/`, `packaging/`, `docs/`, README | Ikonica, bundle konfiguracija (dmg, NSIS, MSI, deb, rpm, AppImage), CI (fmt/clippy/test/UI), release workflow, licence, uputstva | – |

## Faze

1. **Faza 0** – istraživanje i dokumentacija ✅
2. **Faza 1** – izbor jezika ✅ (Rust + Tauri v2, vidi [01-istrazivanje-jezika.md](01-istrazivanje-jezika.md#7-konačna-odluka-rust--tauri-v2))
3. **Faza 2** – skelet workspace-a + API ugovor ✅
4. **Faza 3** – T1–T7 paralelno
5. **Faza 4** – integracija: `cargo clippy --workspace -D warnings`, `cargo test --workspace`,
   `npm run check`, build na Linuksu, ručni test u mock režimu, pregled koda
6. **Faza 5** – push, tag `v0.1.0` → GitHub Actions (`release.yml`) pravi Release sa
   instalacijama za macOS (universal), Windows x64, Linux x64 i arm64 i CLI binarkama

## CI/CD

* `.github/workflows/ci.yml` – na svaki push/PR: Linux (fmt, clippy `-D warnings`, testovi,
  UI `check` + `build`) i `cargo check` na macOS-u i Windows-u.
* `.github/workflows/release.yml` – na tag `vX.Y.Z` (ili ručno): verzija iz taga se upisuje
  u `Cargo.toml`, `tauri.conf.json` i `ui/package.json` (`scripts/set-version.mjs`),
  `tauri-apps/tauri-action` gradi i kači instalacije, CLI se gradi za svaku platformu
  (macOS universal preko `lipo`), a zatim se draft release objavljuje.

## Rizici

* **Nema fizičkog čitača u CI** – parsiranje se testira golden podacima iz Baš Čelika;
  UI i backend se testiraju mock režimom (`CHITACH_MOCK=1` / `npm run dev` u browseru).
* **Port bez kartice za proveru** – logika se prenosi 1:1 iz Baš Čelika (iste APDU
  sekvence i konstante); greške prijavljene od korisnika se porede sa ponašanjem originala.
* **macOS potpisivanje** – bez Apple Developer naloga build je ad-hoc potpisan i nije
  notarizovan (Gatekeeper: "Open Anyway").
* **Windows SmartScreen** – nepotpisan instaler daje upozorenje dok se ne nabavi
  code-signing sertifikat.
* **Linux** – zavisi od `pcscd` servisa i drajvera čitača (`libccid` / `pcsc-lite-ccid`);
  `.deb` i `.rpm` ih navode kao zavisnosti.
