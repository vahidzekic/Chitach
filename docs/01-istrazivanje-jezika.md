# Istraživanje: izbor jezika i tehnologije

## 1. Šta radi Baš Čelik (polazna tačka)

[Baš Čelik](https://github.com/ubavic/bas-celik) (MIT licenca) je čitač srpskih
elektronskih dokumenata sa pametnih kartica:

| Funkcija | Opis |
|---|---|
| Lična karta | Lični podaci, adresa, fotografija, sertifikati, promena PIN-a |
| Zdravstvena knjižica | Podaci o osiguraniku + "overena do" sa RFZO web servisa |
| Saobraćajna dozvola | Podaci o vozilu i vlasniku |
| Izvoz | PDF (izgled kao zvanični), JSON, XLSX |
| Ostalo | Automatsko čuvanje, system tray, Smartbox emulacija (ePorezi), potpisivanje PDF-a preko PKCS#11 |

Tehnički: ~13.000 linija Go koda. GUI je **Fyne** (sopstveno OpenGL iscrtavanje, ne
izgleda nativno). Komunikacija sa karticom ide preko **PC/SC** API-ja
(`winscard.dll` na Windows-u, `PCSC.framework` na macOS-u, `pcsc-lite` na Linuksu)
slanjem APDU komandi, a odgovori se parsiraju kao TLV / BER strukture.

Paketi `card`, `document` i `localization` su **javni Go paketi** (`github.com/ubavic/bas-celik/v2/...`)
i mogu se koristiti kao biblioteka — to je ključna činjenica za izbor jezika.

## 2. Zahtevi za Chitach

1. Desktop aplikacija za **macOS (Intel + Apple Silicon), Linux (x64 + arm64), Windows (x64)**.
2. Nov, drugačiji i moderniji UI.
3. "Nativna" aplikacija — jedan izvršni fajl/instalacija po platformi, bez potrebe da korisnik instalira runtime (Java, .NET, Python...).
4. GitHub Releases sa instalacijom za svaki OS (`.dmg`, `.msi`/`.exe`, `.deb`/`.rpm`/`.AppImage`).

## 3. Postoji li jedan jezik koji je nativan na sve tri platforme?

Da — u smislu da se **kompajlira u nativni mašinski kod** za svaki OS i arhitekturu:
**Go, Rust, C/C++, Swift (samo delimično), C# (NativeAOT), Dart (Flutter)**.
Svi oni prave pravi nativni izvršni fajl bez interpretera.

"Nativni UI" (pravi sistemski widget-i: Cocoa / Win32 / GTK) iz jednog koda daju samo
wxWidgets i (delimično) Qt. U praksi danas se kao standard koristi **nativni backend +
sistemski WebView** (WKWebView na macOS-u, WebView2 na Windows-u, WebKitGTK na Linuksu) —
aplikacija je i dalje nativna binarka (5–15 MB), ali UI se piše HTML/CSS-om što daje
potpunu slobodu dizajna.

## 4. Kandidati

| # | Stek | UI pristup | Pristup kartici | Ponovna upotreba Baš Čelik koda | Veličina | Zrelost | Procena |
|---|---|---|---|---|---|---|---|
| A | **Go + Wails v2** | Sistemski WebView + Svelte/TS | `ebfe/scard` (već koristi Baš Čelik) | **~100% jezgra (import kao biblioteka)** | ~10–15 MB | Stabilan (v2.16) | ⭐ **Preporuka** |
| B | Rust + Tauri v2 | Sistemski WebView + Svelte/TS | `pcsc` crate | 0% — prepisivanje ~5k linija parsiranja + PDF | ~5–10 MB | Stabilan | Odlično, ali sporije i rizičnije |
| C | Go + Fyne | Sopstveno iscrtavanje | `ebfe/scard` | 100% | ~25 MB | Stabilan | Isto kao original — nije "drugačiji UI" |
| D | C# .NET 8 + Avalonia (NativeAOT) | Sopstveno iscrtavanje (XAML) | `PCSC` NuGet | 0% | ~30–60 MB | Stabilan | Dobar, ali veći i prepisivanje |
| E | Dart + Flutter | Sopstveno iscrtavanje | slabija `pcsc` podrška (FFI) | 0% | ~30 MB | Stabilan | Rizik kod PC/SC |
| F | C++ + Qt 6 | Qt widgeti / QML | direktan PC/SC | 0% | ~40 MB | Vrlo zreo | LGPL/komercijalna licenca, najsporiji razvoj |
| G | Rust + Slint / egui | Sopstveno iscrtavanje | `pcsc` crate | 0% | ~5–10 MB | Srednje | Manje fleksibilan UI |
| H | Electron (JS) | Ugrađen Chromium | `pcsclite` (native addon) | 0% | ~150 MB | Zreo | Nije "nativno", ogromno |

## 5. Detaljnije o dva najjača kandidata

### A) Go + Wails v2 (preporuka)

* **Backend u Go-u** importuje `card`, `document`, `localization` pakete iz Baš Čelika —
  čitanje kartica, parsiranje, PDF/JSON/XLSX izvoz rade od prvog dana i već su
  istestirani (golden testovi). Mi pišemo samo svoj sloj (servis, događaji, podešavanja) i UI.
* **Frontend**: Svelte + TypeScript + Vite — potpuno nov dizajn (sidebar, kartice, tamna/svetla tema, animacije).
* Wails automatski generiše TS vezu ka Go metodama i šalje događaje (kartica ubačena/izvađena).
* Pakovanje: `wails build` → `.app` (macOS, universal), `.exe` + **NSIS instalacija** (Windows), Linux binarka → `.deb`/`.rpm` (nfpm) + `.AppImage`.
* Mane: na Linuksu zahteva `libwebkit2gtk-4.1` (rešava se zavisnošću u `.deb`/`.rpm`); Windows 10 stariji od 2022 možda nema WebView2 (NSIS instaler ga automatski instalira).

### B) Rust + Tauri v2

* Najmanje binarke, odlična bezbednost, ugrađeni bundler (`.dmg`, `.msi`, `.deb`, `.rpm`, `.AppImage`) i auto-update.
* Mora se **iz početka napisati** APDU/TLV/BER parsiranje za 3 tipa kartica (+ Gemalto/Apollo varijante ličnih karata), PDF generisanje sa istim izgledom, XLSX, Smartbox server. Bez fizičke kartice za testiranje, rizik od grešaka je značajan (golden test podaci iz Baš Čelika mogu da se iskoriste).
* Procena: 2–3× više posla od opcije A.

## 6. Zaključak

| Kriterijum | Go + Wails | Rust + Tauri |
|---|---|---|
| Brzina razvoja | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| Pouzdanost čitanja kartica | ⭐⭐⭐⭐⭐ (proveren kod) | ⭐⭐⭐ (nov kod) |
| Sloboda UI dizajna | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Veličina aplikacije | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Instalacije za sve OS | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

**Preporuka: Go + Wails v2 + Svelte/TypeScript.**

## 7. Konačna odluka: Rust + Tauri v2

Nakon pregleda ovog istraživanja korisnik je izabrao **opciju B — Rust + Tauri v2 +
Svelte 5/TypeScript**, umesto preporučene Go + Wails varijante. Obrazloženje:

* **Jedan jezik za celo jezgro, bez zavisnosti od tuđeg Go modula.** Protokol kartica
  (APDU, TLV/BER, ATR, Gemalto/Apollo lične karte, zdravstvena, saobraćajna), PDF/JSON/XLSX
  izvoz, RFZO klijent, Smartbox server i XML potpisivanje se **prenose (port) iz Baš Čelika**
  u sopstvene Rust crate-ove (`celik-core`, `celik-export`, `celik-sign`). Kod ostaje pod
  MIT licencom uz navođenje autora originala (vidi `NOTICE`).
* **Manje i brže binarke**, bez garbage collector-a; memorijska bezbednost Rust-a je bitna
  za kod koji parsira binarne podatke sa kartice.
* **Tauri v2 bundler** pravi sve tražene instalacije iz jedne konfiguracije
  (`.dmg`, NSIS `.exe`, `.msi`, `.deb`, `.rpm`, `.AppImage`), a zvanična
  `tauri-apps/tauri-action` GitHub akcija ih automatski kači na GitHub Release.
* **Stabilan i aktivno održavan ekosistem**: `pcsc` crate (PC/SC na sva tri OS-a),
  `cryptoki` (PKCS#11), `pdf-writer` + `rust_xlsxwriter` (izvoz), `tungstenite` (Smartbox websocket).
* **Rizik prepisivanja** (navedeno u tabeli iznad) ublažen je tako što se:
  1. logika prenosi 1:1 iz Baš Čelika (isti algoritmi, iste konstante),
  2. koriste test podaci (golden fajlovi) iz Baš Čelika za jedinične testove,
  3. UI razvija nezavisno uz mock režim (`CHITACH_MOCK=1`) bez fizičkog čitača.

Rezultujuća arhitektura je opisana u [02-arhitektura.md](02-arhitektura.md), a plan
rada u [03-plan-implementacije.md](03-plan-implementacije.md).
