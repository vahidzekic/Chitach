# Arhitektura (Rust + Tauri v2)

Chitach je Cargo workspace sa četiri Rust biblioteke/programa, Tauri v2 aplikacijom i
Svelte 5 + TypeScript frontendom. Jezgro (protokol kartica, izvoz, potpisivanje) je
podeljeno u tri crate-a: `celik-core`, `celik-export` i `celik-sign`.

```
┌──────────────────────────── Chitach.app / Chitach.exe / chitach ─────────────────────────────┐
│  ui/  (Svelte 5 + TS + Vite, sistemski WebView: WKWebView | WebView2 | WebKitGTK)              │
│   ├─ Početni ekran: status čitača, "Ubacite karticu"                                           │
│   ├─ Prikaz dokumenta: Lična karta | Zdravstvena | Saobraćajna (polja, fotografija)           │
│   ├─ Akcije: PDF / JSON / XLSX, kopiranje, RFZO ažuriranje, PIN, sertifikati                  │
│   ├─ Podešavanja: jezik (sr-Latn, sr-Cyrl, en, ru), tema, mod, auto-čuvanje, PKCS#11 putanje  │
│   └─ Modovi: Čitač | Smartbox (ePorezi) | Potpisivanje XML-a                                  │
│            ▲ invoke("komanda", …)                       ▲ listen("app-state" | "notify" | …)   │
├────────────┼─────────────────────────────────────────────┼──────────────────────────────────────┤
│  src-tauri/ (crate `chitach`, lib `chitach_lib`)                                               │
│   commands.rs  Tauri komande (docs/04-api-ugovor.md)                                           │
│   state.rs     Shared: AppState + Settings iza Mutex-a                                         │
│   poller.rs    nit "chitach-poller": prati čitače/kartice, čita dokument                      │
│   events.rs    emitovanje "app-state", "notify", "smartbox-status"                            │
│   export.rs    auto-čuvanje PDF-a, izvoz u fajl                                                │
│   smartbox.rs  pokretanje/zaustavljanje Smartbox servera                                       │
│   signing.rs   PKCS#11 sesija za XML potpisivanje                                              │
│   settings.rs  settings.json u konfiguracionom direktorijumu                                   │
│   tray.rs, update.rs, rfzo.rs                                                                  │
├──────────────┬─────────────────────────┬────────────────────────────────┬────────────────────┤
│ celik-core   │ celik-export            │ celik-sign                     │ chitach-cli         │
│ PC/SC, APDU, │ PDF (pdf-writer +       │ PKCS#11 (cryptoki), XML-DSig,  │ komandna linija     │
│ TLV/BER, ATR,│ Liberation Sans), JSON, │ Smartbox websocket server      │ (core + export)     │
│ Gemalto/     │ XLSX (rust_xlsxwriter)  │ (tungstenite)                  │                     │
│ Apollo/Med./ │                         │                                │                     │
│ Vehicle,     │                         │                                │                     │
│ Document,    │                         │                                │                     │
│ lokalizacija,│                         │                                │                     │
│ RFZO klijent │                         │                                │                     │
├──────────────┴─────────────────────────┴────────────────────────────────┴────────────────────┤
│  PC/SC: winscard.dll | PCSC.framework | libpcsclite (pcscd)     PKCS#11: modul izdavaoca kartice │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Crate-ovi

| Crate | Putanja | Odgovornost |
|---|---|---|
| `celik-core` | `crates/celik-core` | PC/SC pristup (`pcsc` feature), APDU, TLV i BER parsiranje, prepoznavanje kartice po ATR-u, čitanje lične karte (Gemalto i Apollo), zdravstvene knjižice i saobraćajne dozvole, model `Document` (serde, JSON = ugovor sa UI-jem), lokalizacija labela, promena PIN-a, RFZO web servis (`rfzo` feature), primer dokumenti za mock režim |
| `celik-export` | `crates/celik-export` | `to_pdf` (izgled kao zvanični obrazac, ugrađeni Liberation Sans fontovi, ćirilične ili latinične labele), `to_json`, `to_xlsx`, predlog imena fajla |
| `celik-sign` | `crates/celik-sign` | PKCS#11 moduli po izdavaocu (MUP, PKS, Pošta, Halcom, E-Smart), XML-DSig potpisivanje, Smartbox server za ePorezi |
| `chitach-cli` | `crates/chitach-cli` | CLI: lista čitača, ATR, izvoz u PDF/JSON/XLSX, RFZO |
| `chitach` | `src-tauri` | Tauri aplikacija: komande, događaji, niti, podešavanja, tray, provera novih verzija |
| UI | `ui` | Svelte 5 + TypeScript + Vite; `ui/dist` se ugrađuje u binarku (`frontendDist`) |

## Tok podataka (čitanje kartice)

```
 korisnik ubaci karticu
          │
          ▼
 ┌─────────────────┐  SCardGetStatusChange / lista čitača (svakih 500 ms)
 │ poller nit       │──────────────────────────────────────────────┐
 │ (chitach-poller) │                                              │
 └────────┬────────┘                                              │
          │ nova kartica (čitač + ATR)                             │
          ▼                                                        │
 celik_core: ATR → tip kartice → APDU komande → TLV/BER → Document │
          │                                                        │
          ▼                                                        ▼
 Shared state (Mutex): status = reading → ready | unknownCard | error, readId++
          │                     │
          │                     └─► auto-čuvanje (celik_export::to_pdf) → "notify"
          ▼
 emit "app-state" (AppState) ─────────► UI: prikaz dokumenta
                                          │
                                          ▼
                     invoke("export_document", { format, path })
                                          │
                                          ▼
                     celik_export::{to_pdf,to_json,to_xlsx} → fajl na disku
```

## Niti i konkurentnost

| Nit | Kad radi | Šta radi |
|---|---|---|
| Glavna (event loop) | uvek | Tauri/WebView, tray, prozor. Nikad ne blokira na PC/SC. |
| `chitach-poller` | mod **Čitač** | Petlja: lista čitača → stanje kartice → čitanje dokumenta; menja `Shared` i emituje `app-state`. U mock režimu (`CHITACH_MOCK=1`) emituje primer dokumente bez PC/SC-a. |
| Smartbox server | mod **Smartbox** | `celik_sign::smartbox::SmartboxServer` sluša na `ws://127.0.0.1:<port>` u sopstvenoj niti, prima zahteve ePorezi portala i potpisuje preko PKCS#11; status ide događajem `smartbox-status`. Zaustavlja se pri izlasku iz aplikacije. |
| Tauri komande | na zahtev UI-ja | Izvršavaju se na Tauri thread pool-u; dele `Shared` (Mutex) sa poller-om. Komande koje pristupaju kartici (PIN, sertifikati, RFZO) otvaraju sopstvenu PC/SC vezu. |

Jedan mod je aktivan u isto vreme (podešavanje `mode`), promena važi posle restarta —
isto kao u Baš Čeliku, jer i poller i Smartbox traže ekskluzivan pristup čitaču.

## Konfiguracija i podaci

* Podešavanja: `settings.json` u konfiguracionom direktorijumu aplikacije
  (Linux `~/.config/rs.chitach.desktop/`, macOS `~/Library/Application Support/rs.chitach.desktop/`,
  Windows `%APPDATA%\rs.chitach.desktop\`).
* Aplikacija ne šalje podatke nigde osim: RFZO servisu (samo na klik "Ažuriraj",
  šalje LBO i broj kartice) i GitHub API-ju (provera nove verzije, ako je uključena).

## Struktura repozitorijuma

```
Chitach/
├─ Cargo.toml                 # workspace (verzija u [workspace.package])
├─ crates/
│  ├─ celik-core/
│  ├─ celik-export/
│  ├─ celik-sign/
│  └─ chitach-cli/
├─ src-tauri/                 # Tauri v2 aplikacija + tauri.conf.json + icons/
├─ ui/                        # Svelte 5 + TS + Vite
├─ assets/                    # fontovi (OFL), rfzo.png, prevodi, ikonica aplikacije
├─ packaging/icon/            # skripta koja crta ikonicu (assets/app-icon.png)
├─ scripts/set-version.mjs    # sinhronizacija verzije iz git taga
├─ docs/
└─ .github/workflows/         # ci.yml (test), release.yml (tag vX.Y.Z → GitHub Release)
```

## Release artefakti (GitHub Releases, tag `vX.Y.Z`)

| OS | Fajl |
|---|---|
| macOS (Intel + Apple Silicon) | `Chitach_X.Y.Z_universal.dmg` |
| Windows x64 | `Chitach_X.Y.Z_x64-setup.exe` (NSIS), `Chitach_X.Y.Z_x64_en-US.msi` |
| Linux x64 | `Chitach_X.Y.Z_amd64.deb`, `Chitach-X.Y.Z-1.x86_64.rpm`, `Chitach_X.Y.Z_amd64.AppImage` |
| Linux arm64 | `Chitach_X.Y.Z_arm64.deb`, `Chitach-X.Y.Z-1.aarch64.rpm`, `Chitach_X.Y.Z_aarch64.AppImage` |
| CLI | `chitach-cli-macos-universal`, `chitach-cli-windows-x64.exe`, `chitach-cli-linux-x64`, `chitach-cli-linux-arm64` |

macOS build se potpisuje Developer ID sertifikatom i notarizuje kada su Apple tajne
podešene u repozitorijumu ([06-razvoj.md](06-razvoj.md#macos-potpisivanje-i-notarizacija));
bez njih je ad-hoc potpisan (`signingIdentity: "-"`), pa važi "Open Anyway" iz
[korisničkog uputstva](05-korisnicko-uputstvo.md). Windows code-signing još nije podešen.
