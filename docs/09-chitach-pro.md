# Chitach Pro: plaćena provera kazni i istorija vozila

> Status: oktobar 2026. **Kod je spojen u `main` (PR #13), ali je zaključavanje isključeno**:
> dok je `STORE_ID = 0`, provera kazni i istorija vozila su besplatne, a UI ne prikazuje ništa
> o Pro-u. Naplata je odložena za neko od sledećih izdanja; uključuje se upisom podataka iz
> Lemon Squeezy naloga (v. §4).

## 1. Odluka

- Naplaćuju se samo dve funkcije: **provera kazni** i **istorija vozila**. Sve ostalo ostaje
  besplatno.
- Model: **jednokratna kupovina** licencnog ključa („Chitach Pro"), bez pretplate.
- Prodaja preko **Lemon Squeezy** (merchant of record: oni naplaćuju i obračunavaju PDV, isplata
  na PayPal ili banku). Stripe ne podržava prodavce iz Srbije; Paddle je rezervna opcija.
- Repo ostaje javan (MIT). Zato je zaključavanje **za poštene kupce, ne zaštita od kopiranja**:
  ko zna da kompajlira može da napravi verziju bez provere. Prava zaštita bi tražila sopstveni
  server preko kog idu provere; to je razmotreno i odbačeno (trošak, održavanje, gubi se
  „tablica ide samo na zvanični portal").

Razmatrane opcije i obrazloženje: thread „Plaćene funkcije" u projektu.

## 2. Šta je urađeno

### Rust (`src-tauri/src/license.rs`)

- Aktivacija, provera i deaktivacija preko **javnog Lemon Squeezy License API-ja**
  (`/v1/licenses/activate`, `validate`, `deactivate`). Ne treba API tajna, ništa tajno nije u
  aplikaciji; šalje se samo ključ i ime instance (npr. `Chitach (windows x86_64)`).
- Prihvataju se samo ključevi prodavnice `STORE_ID` (i proizvoda iz `PRODUCT_IDS`, ako je
  lista neprazna). Ključ drugog proizvoda se odbija, a zauzeta aktivacija se odmah oslobađa.
- Ključ se čuva u `license.json` pored podešavanja (na Unix-u prava 0600) i nikad se ne šalje UI-ju
  (UI vidi samo poslednja 4 znaka).
- Ponovna provera u pozadini pri startu ako je od poslednje prošlo **više od 7 dana**. Bez
  interneta Pro radi još **30 dana** od poslednje uspešne provere. Refundiran ili onemogućen
  ključ se briše.
- Zaključane komande vraćaju `error.proRequired`: `fines_check` i `history_api_check`.
- Nove komande: `license_info`, `license_activate`, `license_deactivate`
  (opis u [04-api-ugovor.md](04-api-ugovor.md)).
- 6 testova (greške aktivacije, provera prodavnice, validacija, period bez interneta, čuvanje,
  mock aktivacija).

### UI

- Bez licence se na saobraćajnoj dozvoli umesto panela za kazne i istoriju prikazuje kartica
  „Provera kazni i istorija vozila su deo Chitach Pro" sa dugmetom „Kupi Pro" i poljem za ključ
  (`ui/src/lib/pages/document/ProLock.svelte`).
- U Podešavanjima je nova sekcija „Chitach Pro": unos ključa, status i deaktivacija na ovom
  računaru.
- Prevodi za sva 4 jezika (`pro.*`, `error.license*`, `error.proRequired`).
- Mock režim: u browseru je zaključavanje isključeno kao u pravom build-u; `?pro=0` ga uključuje
  i prikazuje zaključano stanje, `?pro=1` aktiviran Pro. U Rust-u (`CHITACH_MOCK=1`) je
  zaključavanje uključeno. U oba radi probni ključ `CHITACH-PRO-DEMO`.

### Dokumentacija i slike

- [04-api-ugovor.md](04-api-ugovor.md): tip `LicenseInfo` i tri nove komande.
- Slike: `screenshots/25-pro-locked.png`, `screenshots/26-pro-settings.png`.

### Provereno

`cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace`,
`npm run check`, `npm run build`, aktivacija probnim ključem u browser mock-u (Playwright).
CI zelen na Linux-u, macOS-u i Windows-u. **Nije probano sa pravim Lemon Squeezy ključem.**

## 3. Šta fali

1. **Podaci iz Lemon Squeezy naloga** u `src-tauri/src/license.rs` (v. §4). Upisom `STORE_ID`
   zaključavanje se uključuje u sledećem build-u.
2. **Proizvod u Lemon Squeezy:** uključiti „License keys", trajanje bez isteka, ograničenje
   aktivacija (predlog: 3 računara), cena (predlog: 5 do 10 €).
3. **Test sa pravim ključem:** kupiti proizvod u test režimu Lemon Squeezy-ja, aktivirati,
   deaktivirati, pa probati refund (ključ treba da nestane pri sledećoj proveri).
4. **Proveriti isplatu** na srpski PayPal ili račun u podešavanjima Lemon Squeezy naloga.
5. **README (sr i en):** dodati stavku „Chitach Pro" i u deo o privatnosti navesti proveru ključa
   kod prodavca (Lemon Squeezy).
6. **Korisničko uputstvo** ([05-korisnicko-uputstvo.md](05-korisnicko-uputstvo.md)): dodati deo o
   kupovini, aktivaciji i prenosu ključa na drugi računar.
7. **Uslovi prodaje i refundacije:** kratka stranica (može na sajtu projekta) na koju vodi
   Lemon Squeezy checkout.
8. Odlučiti u kom izdanju izlazi (provera kazni je planirana za v0.2.0) i da li te funkcije
   prvo izlaze besplatno, pa tek kasnije kao Pro.

## 4. Gde se upisuju podaci

Na vrhu `src-tauri/src/license.rs`:

```rust
pub const STORE_ID: u64 = 0;          // Settings → Stores, broj pored imena prodavnice
pub const PRODUCT_IDS: &[u64] = &[];  // Products → ⋯ → Copy ID (npr. &[123456])
pub const BUY_URL: &str = "";         // Products → Share → link za kupovinu
```

Ovi podaci nisu tajni. **Lemon Squeezy API ključ nije potreban** i ne sme da se upisuje u kod.
