# Razvoj

## Preduslovi

| Alat | Verzija |
|---|---|
| Rust | stable (MSRV 1.80), `rustup` |
| Node.js | 22 (npm 10) |
| Tauri CLI | dolazi iz `ui/node_modules` (`@tauri-apps/cli`); opciono `cargo install tauri-cli --locked` za `cargo tauri` |

Sistemske biblioteke:

* **Debian/Ubuntu**
  ```sh
  sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
    librsvg2-dev libayatana-appindicator3-dev libssl-dev libxdo-dev \
    libpcsclite-dev pcscd libccid
  ```
* **Fedora**
  ```sh
  sudo dnf install webkit2gtk4.1-devel gtk3-devel librsvg2-devel libappindicator-gtk3-devel \
    openssl-devel libxdo-devel pcsc-lite-devel pcsc-lite pcsc-lite-ccid
  ```
* **macOS**: Xcode Command Line Tools (`xcode-select --install`).
* **Windows**: Visual Studio Build Tools (C++ workload) i WebView2 (već postoji na Windows 11).

## Struktura

Vidi [02-arhitektura.md](02-arhitektura.md). Ukratko: `crates/*` (biblioteke i CLI),
`src-tauri` (Tauri aplikacija), `ui` (Svelte 5 + TS). Ugovor komandi i događaja:
[04-api-ugovor.md](04-api-ugovor.md).

## Pokretanje u razvoju

```sh
npm --prefix ui ci              # jednom

# samo UI u pretraživaču (mock API sa primer dokumentima, bez Rust-a)
npm --prefix ui run dev         # http://localhost:5173

# cela aplikacija (Tauri sam pokreće Vite dev server, hot reload UI-ja)
npm --prefix ui exec -- tauri dev
# ili, ako je instaliran tauri-cli:
cargo tauri dev
```

### Mock režim (bez čitača)

```sh
CHITACH_MOCK=1 npm --prefix ui exec -- tauri dev
# Windows PowerShell:  $env:CHITACH_MOCK=1; npm --prefix ui exec -- tauri dev
```

Backend tada ne koristi PC/SC, već prikazuje virtuelni čitač ("Mock Reader") i primer
dokumente (`celik_core::sample`) — korisno za razvoj UI-ja i screenshot-ove.

## Provere (isto što radi CI)

```sh
npm --prefix ui run check
npm --prefix ui run build       # src-tauri ugrađuje ui/dist, pa mora postojati pre cargo build-a
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Build

```sh
npm --prefix ui exec -- tauri build                 # instalacije za trenutni OS
npm --prefix ui exec -- tauri build --bundles deb   # samo .deb
npm --prefix ui exec -- tauri build --target universal-apple-darwin   # macOS universal
cargo build --release -p chitach-cli                # CLI
```

`tauri build` sam pokreće `npm run build` u `ui/` (`beforeBuildCommand`). Rezultati su u
`target/release/bundle/` (ili `target/<triple>/release/bundle/`).

Konfiguracija pakovanja je u `src-tauri/tauri.conf.json` (`bundle`): zavisnosti `.deb`
(`libpcsclite1`, `pcscd`, preporuka `libccid`) i `.rpm` (`pcsc-lite`, `pcsc-lite-ccid`),
NSIS (instalacija za korisnika ili za sve), WebView2 bootstrapper, MSI, macOS 10.15+ sa
ad-hoc potpisom.

### Ikonica

Izvor je `assets/app-icon.png` (1024×1024), nacrtana skriptom
`packaging/icon/make_icon.py` (Python + Pillow). Posle izmene:

```sh
python3 packaging/icon/make_icon.py assets/app-icon.png
cd ui && npx tauri icon ../assets/app-icon.png -o ../src-tauri/icons
rm -rf ../src-tauri/icons/android ../src-tauri/icons/ios
```

## Izdavanje nove verzije

1. Proverite da je `main` zelen u CI-ju i ažurirajte dokumentaciju po potrebi.
2. Napravite i pošaljite tag:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. Workflow **Release** (`.github/workflows/release.yml`):
   * upisuje verziju iz taga u `Cargo.toml`, `src-tauri/tauri.conf.json` i
     `ui/package.json` (`node scripts/set-version.mjs v0.2.0` — isto možete pokrenuti
     lokalno i commit-ovati da bi verzija u repozitorijumu bila ažurna),
   * pravi draft release "Chitach v0.2.0" sa automatski generisanim beleškama i tabelom
     fajlova po OS-u,
   * gradi i kači instalacije za macOS (universal), Windows x64, Linux x64 i Linux arm64,
     kao i `chitach-cli-<os>-<arch>` binarke,
   * objavljuje release kada su svi build-ovi uspešni.
4. Release se može pokrenuti i ručno (*Actions → Release → Run workflow*, uz unos taga).

Napomene:
* Verzija mora biti oblika `X.Y.Z`. Pre-release tagovi (`v1.0.0-rc.1`) se objavljuju kao
  pre-release, ali MSI (WiX) ne prihvata pre-release verzije — za njih koristite samo
  `X.Y.Z`.
* Potpisivanje: macOS je ad-hoc potpisan (`signingIdentity: "-"`). Za notarizaciju dodajte
  tajne `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` i prosledite ih `tauri-action` koraku.

## Doprinos

Vidi [CONTRIBUTING.md](../CONTRIBUTING.md).
