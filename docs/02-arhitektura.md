# Arhitektura (za preporučenu varijantu Go + Wails)

```
┌──────────────────────────── Chitach.app / Chitach.exe / chitach ────────────────────────────┐
│  Frontend (Svelte + TS, sistemski WebView)                                                   │
│   ├─ Početni ekran: status čitača, animacija "Ubacite karticu"                               │
│   ├─ Prikaz dokumenta: Lična karta | Zdravstvena | Saobraćajna (kartice sa poljima, foto)    │
│   ├─ Akcije: Sačuvaj PDF / JSON / XLSX, kopiraj polje, RFZO ažuriranje, promena PIN-a        │
│   └─ Podešavanja: jezik (sr-Latn, sr-Cyrl, en), tema, auto-čuvanje, pismo PDF-a              │
│                ▲ Wails bindings (generisani TS)        ▲ Wails events                         │
├────────────────┼─────────────────────────────────────────┼───────────────────────────────────┤
│  Go backend    │                                         │                                   │
│   app/         │ App servis: ListReaders, SelectReader, Export*, ChangePin, Settings          │
│   reader/      │ Poller: prati čitače i kartice (SCardGetStatusChange), emituje događaje      │
│   model/       │ DTO za UI (polja, grupe, slika base64) — nezavisno od Baš Čelik tipova       │
│   settings/    │ JSON u korisničkom config direktorijumu (os.UserConfigDir)                   │
│   i18n/        │ prevodi labela                                                               │
├────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│  Jezgro (biblioteka): github.com/ubavic/bas-celik/v2/{card,document,localization} — MIT      │
├──────────────────────────────────────────────────────────────────────────────────────────────┤
│  PC/SC: winscard.dll | PCSC.framework | libpcsclite                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

Takođe: `chitach-cli` (isti backend, bez UI-ja) za `--list`, `--pdf`, `--json`, `--xlsx`.

## Struktura repozitorijuma

```
Chitach/
├─ main.go, app.go           # Wails ulaz
├─ internal/
│  ├─ core/                  # adapter ka bas-celik paketima (čitanje, izvoz)
│  ├─ reader/                # poller čitača/kartica
│  ├─ model/                 # DTO za frontend
│  └─ settings/
├─ cmd/chitach-cli/          # CLI verzija
├─ frontend/                 # Svelte + Vite + TS
├─ build/                    # ikone, Info.plist, NSIS, .desktop
├─ packaging/                # nfpm (deb/rpm), AppImage
├─ docs/
└─ .github/workflows/        # CI (test) + release (tag v*)
```

## Release artefakti (GitHub Releases, na tag `vX.Y.Z`)

| OS | Fajl |
|---|---|
| macOS (Intel + Apple Silicon) | `Chitach-X.Y.Z-macos-universal.dmg` |
| Windows x64 | `Chitach-X.Y.Z-windows-amd64-setup.exe` (NSIS) + portable `.zip` |
| Linux x64 / arm64 | `.deb`, `.rpm`, `.AppImage`, `.tar.gz` |
| CLI | `chitach-cli-<os>-<arch>` |

Napomena: macOS build je ad-hoc potpisan (bez Apple Developer naloga korisnik mora
desni klik → Open prvi put). Notarizacija je moguća kasnije dodavanjem tajni u GitHub.
