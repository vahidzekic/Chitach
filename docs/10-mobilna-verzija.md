# Mobilna verzija (Android, kasnije iOS)

Stanje: faza 1, Android osnova. Ništa nije objavljeno u Google Play ni App Store.

## Kako telefon čita kartice

Lična karta, zdravstvena knjižica i saobraćajna dozvola imaju samo kontaktni čip (bez NFC-a),
pa telefon ne može da ih pročita prislanjanjem. Potreban je USB čitač kartica (standardni
CCID čitač, isti kao za računar) uključen u telefon preko USB-C priključka ili OTG adaptera.

Android nema PC/SC servis, pa aplikacija sama priča sa čitačem preko USB host API-ja:

```
UI (Svelte, isti kao na desktopu)
  │ invoke / app-state
src-tauri (Rust)
  poller.rs   run_usb: svakih 500 ms lista čitača, traži USB dozvolu, čita karticu
  transport.rs connect(): desktop = PC/SC (PcscCard), telefon = UsbCard
  │ run_mobile_plugin("readers" | "requestPermission" | "connect" | "transmit")
crates/tauri-plugin-ccid
  src/lib.rs                        Rust API (Ccid::readers, connect, transmit)
  android/.../CcidPlugin.kt         CCID preko USB-a: GetSlotStatus, IccPowerOn (ATR), XfrBlock (APDU)
```

Celo čitanje kartice (`celik-core`) ide kroz `CardTransport`, pa je isto na svim platformama.
Obrada `61xx`/`6Cxx` odgovora je u `celik_core::card::transmit_with_status_handling` i koriste je
oba transporta.

Podržani su čitači koji razmenjuju cele APDU komande (short ili extended APDU nivo u CCID
deskriptoru), što je većina današnjih USB čitača. Čitači koji rade samo na TPDU nivou dobijaju
poruku `error.readerUnsupported`; za njih bi trebalo dodati T=1 protokol u Kotlin deo.

## Šta radi na telefonu

| Funkcija | Telefon |
|---|---|
| Čitanje sve tri kartice | da, uz USB čitač |
| RFZO, PIN, sertifikati | da |
| Provera kazni, istorija vozila | da |
| Izvoz PDF/JSON/XLSX | još ne (faza 2: Android dijalog vraća `content://` adresu, treba upis preko nje ili "Podeli") |
| Smartbox, potpisivanje XML-a, ikonica u traci, auto-čuvanje, provera novih verzija | ne, samo desktop; podešavanja ih na telefonu ne prikazuju |

Na uskom ekranu (do 720 px) bočni meni postaje traka na vrhu. Snimci ekrana:
`docs/screenshots/android-*.png` (demo režim u pregledaču, širina Pixel 7).

## Build

GitHub Actions (`.github/workflows/android.yml`) na svakom push-u pravi debug APK (arm64) i
stavlja ga kao artefakt `chitach-android-debug-apk`. Debug APK se može instalirati na telefon za
probu (uključiti instalaciju iz nepoznatih izvora), ali nije za Google Play.

Android Studio projekat (`src-tauri/gen/android`) se za sada generiše u CI-ju
(`tauri android init --ci`), ne čuva se u repozitorijumu. Identifikator na telefonu je
`rs.chitach.app` (`src-tauri/tauri.android.conf.json`); desktop ostaje `rs.chitach.desktop`
da se ne bi promenio folder sa podešavanjima.

Lokalno (potrebni Android SDK, NDK i Java 17):

```sh
cd ui && npm ci && cd ..
export ANDROID_HOME=... NDK_HOME=...
npm run tauri -- android init
npm run tauri -- android dev        # telefon povezan kablom, uključen USB debugging
```

## Sledeće faze

1. Proba na pravom telefonu sa čitačem; izvoz preko Android "Podeli"; demo režim za recenzente.
2. Google Play: nalog, politika privatnosti, Data safety, potpisivanje (Play App Signing),
   zatvoreni test 12 testera / 14 dana, pa objava tek uz odobrenje.
3. iOS: Swift deo plugina preko CryptoTokenKit (`TKSmartCard`, iOS 16+), TestFlight, App Store.
4. Chitach Pro na telefonu ide preko Apple/Google pretplate (pravila prodavnica), ne preko
   Lemon Squeezy linka.
