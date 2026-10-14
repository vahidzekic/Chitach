<p align="center">
  <img src="assets/app-icon.png" width="128" alt="Chitach">
</p>

<h1 align="center">Chitach</h1>

<p align="center">
  Čitač srpskih elektronskih dokumenata — lična karta, zdravstvena knjižica, saobraćajna dozvola<br>
  za <b>Windows</b>, <b>macOS</b> i <b>Linux</b>
</p>

<p align="center">
  <a href="https://github.com/vahidzekic/Chitach/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/vahidzekic/Chitach"></a>
  <a href="https://github.com/vahidzekic/Chitach/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/vahidzekic/Chitach/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/licenca-MIT-blue"></a>
</p>

<p align="center"><a href="README.en.md">English</a></p>

---

**Chitach** je besplatan program otvorenog koda za čitanje srpskih dokumenata sa
pametnih kartica, sa novim, modernim interfejsom. Napisan je u Rust-u (Tauri v2), a
logika čitanja kartica i izvoza je preneta iz odličnog projekta
[Baš Čelik](https://github.com/ubavic/bas-celik) Nikole Ubavića.

## Mogućnosti

* **Lična karta** — svi modeli sa čipom (Gemalto i Apollo): lični podaci, prebivalište,
  fotografija, podaci o dokumentu
* **Zdravstvena knjižica** — podaci o osiguraniku i osiguranju, **"overena do"** preuzima
  se sa RFZO servisa jednim klikom
* **Saobraćajna dozvola** — podaci o vozilu, vlasniku i korisniku
* Izvoz u **PDF** (izgled kao zvanični obrazac, latinične ili ćirilične labele),
  **JSON** i **XLSX**
* Automatsko prepoznavanje kartice i čitača, **automatsko čuvanje** PDF-a pri čitanju
* Promena **PIN-a** lične karte, pregled i čuvanje **sertifikata**
* **Smartbox** emulacija za prijavu na **ePorezi** i **potpisivanje XML-a** (PKCS#11)
* Svetla i tamna tema, jezici: srpski (latinica i ćirilica), engleski, ruski
* Sistemska traka (rad u pozadini), provera novih verzija
* **`chitach-cli`** za rad iz komandne linije
* Podaci ne napuštaju vaš računar (osim upita RFZO-u na vaš zahtev)

## Preuzimanje

Najnovija verzija: **[github.com/vahidzekic/Chitach/releases/latest](https://github.com/vahidzekic/Chitach/releases/latest)**

| Sistem | Fajl |
|---|---|
| **Windows 10/11** (x64) | `Chitach_X.Y.Z_x64-setup.exe` (ili `.msi`) |
| **macOS 10.15+** (Intel i Apple Silicon) | `Chitach_X.Y.Z_universal.dmg` |
| **Ubuntu / Debian / Mint** (x64 / arm64) | `Chitach_X.Y.Z_amd64.deb` / `Chitach_X.Y.Z_arm64.deb` |
| **Fedora / RHEL / openSUSE** (x64 / arm64) | `Chitach-X.Y.Z-1.x86_64.rpm` / `Chitach-X.Y.Z-1.aarch64.rpm` |
| **Ostali Linux** (x64 / arm64) | `Chitach_X.Y.Z_amd64.AppImage` / `Chitach_X.Y.Z_aarch64.AppImage` |
| **Komandna linija** | `chitach-cli-windows-x64.exe`, `chitach-cli-macos-universal`, `chitach-cli-linux-x64`, `chitach-cli-linux-arm64` |

> **macOS:** aplikacija nije notarizovana — prvi put je otvorite desnim klikom → *Open*, ili
> u *System Settings → Privacy & Security* kliknite *Open Anyway*.
> **Linux:** potreban je servis `pcscd`: `sudo systemctl enable --now pcscd`.
> **Windows:** ako SmartScreen upozori, kliknite *More info → Run anyway*.

Detaljno uputstvo za instalaciju i upotrebu: **[docs/05-korisnicko-uputstvo.md](docs/05-korisnicko-uputstvo.md)**.

## Izgled

| Lična karta | Zdravstvena knjižica |
|---|---|
| ![Lična karta](docs/screenshots/02-id-card.png) | ![Zdravstvena knjižica](docs/screenshots/03-medical.png) |
| **Saobraćajna dozvola** | **Čekanje na karticu** |
| ![Saobraćajna dozvola](docs/screenshots/04-vehicle.png) | ![Čekanje na karticu](docs/screenshots/01-waiting.png) |
| **Podešavanja** | **Tamna tema** |
| ![Podešavanja](docs/screenshots/05-settings.png) | ![Tamna tema](docs/screenshots/06-dark-id-card.png) |

*Na slikama su izmišljeni primer podaci.*

## Komandna linija

```sh
chitach-cli --list                    # spisak čitača
chitach-cli --pdf licna.pdf           # sačuvaj PDF
chitach-cli --json podaci.json        # sačuvaj JSON
chitach-cli --help                    # sve opcije
```

## Build iz izvornog koda

Potrebni su Rust (stable), Node.js 22 i sistemske biblioteke za Tauri i PC/SC
(na Linuksu `libwebkit2gtk-4.1-dev`, `libpcsclite-dev`… — vidi [docs/06-razvoj.md](docs/06-razvoj.md)).

```sh
git clone https://github.com/vahidzekic/Chitach.git
cd Chitach
npm --prefix ui ci
npm --prefix ui exec -- tauri dev       # razvoj (CHITACH_MOCK=1 za rad bez čitača)
npm --prefix ui exec -- tauri build     # instalacije za trenutni OS
cargo build --release -p chitach-cli    # CLI
```

## Dokumentacija

* [Korisničko uputstvo](docs/05-korisnicko-uputstvo.md)
* [Razvoj, build i izdavanje verzija](docs/06-razvoj.md)
* [Arhitektura](docs/02-arhitektura.md)
* [API ugovor (Rust ↔ UI)](docs/04-api-ugovor.md)
* [Plan implementacije](docs/03-plan-implementacije.md)
* [Istraživanje izbora jezika](docs/01-istrazivanje-jezika.md)
* [Doprinos projektu](CONTRIBUTING.md)

## Licenca i zahvalnice

Chitach je objavljen pod [MIT licencom](LICENSE).

* Logika čitanja kartica, PDF izgled, RFZO, Smartbox i prevodi su preneti iz projekta
  **[Baš Čelik](https://github.com/ubavic/bas-celik)** — © 2023–2026 Nikola Ubavić, MIT licenca.
* Fontovi **Liberation Sans** — SIL Open Font License 1.1 ([assets/fonts/LICENSE-OFL.txt](assets/fonts/LICENSE-OFL.txt)).

Detalji u [NOTICE](NOTICE). Chitach nije zvanična aplikacija MUP-a, RFZO-a niti bilo koje
državne institucije.
