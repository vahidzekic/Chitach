# Istraživanje: izbor jezika i tehnologije

## 1. Funkcionalnosti koje Chitach treba da podrži

Chitach je čitač srpskih elektronskih dokumenata sa pametnih kartica:

| Funkcija | Opis |
|---|---|
| Lična karta | Lični podaci, adresa, fotografija, podaci o dokumentu (Gemalto i Apollo modeli) |
| Zdravstvena knjižica | Podaci o osiguraniku + "overena do" sa RFZO web servisa |
| Saobraćajna dozvola | Podaci o vozilu, vlasniku i korisniku |
| Izvoz | PDF (izgled kao zvanični obrazac), JSON, XLSX |
| PIN i sertifikati | Promena PIN-a lične karte, pregled i čuvanje sertifikata |
| ePorezi | Smartbox emulacija (lokalni websocket server za prijavu na ePorezi portal) |
| Potpisivanje | Potpisivanje XML dokumenata tokenom preko PKCS#11 |
| CLI | Komandna linija za čitanje kartice i izvoz bez grafičkog interfejsa |
| Ostalo | Automatsko prepoznavanje čitača i kartice, automatsko čuvanje, system tray |

Tehnički: komunikacija sa karticom ide preko **PC/SC** API-ja
(`winscard.dll` na Windows-u, `PCSC.framework` na macOS-u, `pcsc-lite` na Linuksu)
slanjem APDU komandi, a odgovori se parsiraju kao TLV / BER strukture. Izabrani jezik
mora da ima pouzdan pristup PC/SC-u i PKCS#11-u na sva tri OS-a.

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

| # | Stek | UI pristup | Pristup kartici | Veličina | Zrelost | Procena |
|---|---|---|---|---|---|---|
| A | Go + Wails v2 | Sistemski WebView + Svelte/TS | `ebfe/scard` | ~10–15 MB | Stabilan (v2.16) | Dobar, GC i cgo za PKCS#11 |
| B | **Rust + Tauri v2** | Sistemski WebView + Svelte/TS | `pcsc` crate | ~5–10 MB | Stabilan | ⭐ **Izbor** |
| C | Go + Fyne | Sopstveno iscrtavanje | `ebfe/scard` | ~25 MB | Stabilan | UI ne izgleda nativno, ograničen dizajn |
| D | C# .NET 8 + Avalonia (NativeAOT) | Sopstveno iscrtavanje (XAML) | `PCSC` NuGet | ~30–60 MB | Stabilan | Dobar, ali veći |
| E | Dart + Flutter | Sopstveno iscrtavanje | slabija `pcsc` podrška (FFI) | ~30 MB | Stabilan | Rizik kod PC/SC |
| F | C++ + Qt 6 | Qt widgeti / QML | direktan PC/SC | ~40 MB | Vrlo zreo | LGPL/komercijalna licenca, najsporiji razvoj |
| G | Rust + Slint / egui | Sopstveno iscrtavanje | `pcsc` crate | ~5–10 MB | Srednje | Manje fleksibilan UI |
| H | Electron (JS) | Ugrađen Chromium | `pcsclite` (native addon) | ~150 MB | Zreo | Nije "nativno", ogromno |

## 5. Detaljnije o dva najjača kandidata

### A) Go + Wails v2

* **Backend u Go-u**, frontend Svelte + TypeScript + Vite.
* Wails automatski generiše TS vezu ka Go metodama i šalje događaje (kartica ubačena/izvađena).
* Pakovanje: `wails build` → `.app` (macOS, universal), `.exe` + **NSIS instalacija** (Windows), Linux binarka → `.deb`/`.rpm` (nfpm) + `.AppImage`.
* Mane: garbage collector, PKCS#11 zahteva cgo (komplikuje cross-kompajliranje); na Linuksu zahteva `libwebkit2gtk-4.1`.

### B) Rust + Tauri v2

* Najmanje binarke, odlična bezbednost, ugrađeni bundler (`.dmg`, `.msi`, `.deb`, `.rpm`, `.AppImage`) i auto-update.
* Memorijska bezbednost je bitna za kod koji parsira binarne podatke sa kartice.
* APDU/TLV/BER parsiranje za 3 tipa kartica (+ Gemalto/Apollo varijante ličnih karata), PDF generisanje, XLSX i Smartbox server pišu se kao sopstveni Rust crate-ovi. Bez fizičke kartice za testiranje, rizik od grešaka se smanjuje golden testovima i virtuelnom karticom.

## 6. Poređenje

| Kriterijum | Go + Wails | Rust + Tauri |
|---|---|---|
| Brzina razvoja | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| Bezbednost parsiranja binarnih podataka | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Sloboda UI dizajna | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Veličina aplikacije | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Instalacije za sve OS | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

## 7. Konačna odluka: Rust + Tauri v2

Izabrana je **opcija B — Rust + Tauri v2 + Svelte 5/TypeScript**. Obrazloženje:

* **Jedan jezik za celo jezgro.** Protokol kartica (APDU, TLV/BER, ATR, Gemalto/Apollo
  lične karte, zdravstvena, saobraćajna), PDF/JSON/XLSX izvoz, RFZO klijent, Smartbox server
  i XML potpisivanje su u sopstvenim Rust crate-ovima (`celik-core`, `celik-export`, `celik-sign`).
* **Manje i brže binarke**, bez garbage collector-a; memorijska bezbednost Rust-a je bitna
  za kod koji parsira binarne podatke sa kartice.
* **Tauri v2 bundler** pravi sve tražene instalacije iz jedne konfiguracije
  (`.dmg`, NSIS `.exe`, `.msi`, `.deb`, `.rpm`, `.AppImage`), a zvanična
  `tauri-apps/tauri-action` GitHub akcija ih automatski kači na GitHub Release.
* **Stabilan i aktivno održavan ekosistem**: `pcsc` crate (PC/SC na sva tri OS-a),
  `cryptoki` (PKCS#11), `pdf-writer` + `rust_xlsxwriter` (izvoz), `tungstenite` (Smartbox websocket).
* **Rizik od grešaka bez fizičke kartice** ublažen je tako što se:
  1. protokol kartica implementira prema specifikaciji (ISO 7816-4 APDU, TLV/BER) uz
     konstante i strukture fajlova pojedinačnih kartica,
  2. koriste golden test podaci (primeri sadržaja fajlova sa kartica) za jedinične testove,
  3. UI razvija nezavisno uz mock režim (`CHITACH_MOCK=1`) bez fizičkog čitača.

Rezultujuća arhitektura je opisana u [02-arhitektura.md](02-arhitektura.md), a plan
rada u [03-plan-implementacije.md](03-plan-implementacije.md).
