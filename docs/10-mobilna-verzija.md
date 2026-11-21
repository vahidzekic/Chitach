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
deskriptoru) i čitači na TPDU nivou (mnogi jeftini i kombinovani SD + smart card čitači). Za TPDU
čitače Kotlin deo sam radi protokol kartice: iz ATR-a šalje SetParameters, pa razmenjuje T=1
blokove (ISO 7816-3, LRC, ulančavanje, WTX, IFS) ili T=0 komande. Samo čitači na nivou karaktera
dobijaju poruku `error.readerUnsupported`.

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

### iOS build (na Mac-u)

Potrebni Xcode (prijavljen na tim MW256GPMA4), Rust targeti `aarch64-apple-ios`, Node i CocoaPods.

```sh
cd ui && npm ci && cd ..
npm run tauri -- ios init
# App Store ne prihvata ikonu sa alfa kanalom: iOS ikone se prave iz neprovidne ikone
npm run tauri -- icon src-tauri/icons/ios-icon-1024.png
git checkout -- src-tauri/icons && git clean -fd src-tauri/icons
for f in src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/*.png; do
  sips -s format jpeg "$f" --out /tmp/x.jpg >/dev/null && sips -s format png /tmp/x.jpg --out "$f" >/dev/null
done
sips -g hasAlpha src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/*.png   # sve "no"
npm run tauri -- ios build --export-method app-store-connect
```

`tauri ios init` pokušava da nadogradi CocoaPods preko Homebrew-a; na Intel Mac-u to znači
kompajliranje LLVM-a (~45 min), pa je bolje zadržati postojeći CocoaPods. Za instalaciju na
telefon kablom Xcode mora da podržava iOS verziju telefona; inače ide preko TestFlight-a.

`src-tauri/Info.ios.plist` uključuje UIScene (UIApplicationSceneManifest), pa prozor prati veličinu
ekrana na iPad-u i na rasklopljenom iPhone Duo-u. Za pun ekran na Duo-u build treba praviti sa
Xcode 27 (iOS 27 SDK); sa starijim SDK-om iOS 27 aplikaciju prikazuje umanjenu.

## Sledeće faze

1. Proba na pravom telefonu sa čitačem; izvoz preko Android "Podeli"; demo režim za recenzente.
2. Google Play: nalog, politika privatnosti, Data safety, potpisivanje (Play App Signing),
   zatvoreni test 12 testera / 14 dana, pa objava tek uz odobrenje.
3. iOS: Swift deo plugina preko CryptoTokenKit (`TKSmartCard`, iOS 16+), TestFlight, App Store.
4. Chitach Pro na telefonu ide preko Apple/Google pretplate (pravila prodavnica), ne preko
   Lemon Squeezy linka.
