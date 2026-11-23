# Chitach Free i Chitach Pro: dva repoa

> Stanje: novembar 2026. **Korak 2 od 8** iz plana razdvajanja: priključak za Pro je u ovom
> repou, a Pro kod je još ovde, u svojim folderima. Seljenje u privatni repo
> `vahidzekic/chitach-pro` je sledeći korak (posle spajanja PR #21).

## Ukratko

- Ovaj repo (javni, MIT) je **Chitach Free**: čitanje tri kartice, PDF, RFZO „overena do“,
  `chitach-cli` i USB čitač na telefonu. Build samo ovog repoa daje Free aplikaciju, bez
  ijednog Pro menija ili zaključanog ekrana.
- **Chitach Pro** (privatni repo) dodaje proveru kazni, istoriju vozila, sertifikate i promenu
  PIN-a, XML potpis, Smartbox, licencu, JSON/Excel izvoz, automatsko čuvanje i rad u pozadini.
- Zvanični buildovi (desktop, Android, iOS) spajaju oba repoa skriptom
  [`scripts/with-pro.sh`](../scripts/with-pro.sh).

## Gde je Pro kod

Privatni repo ima iste putanje kao ovaj. Pri zvaničnom buildu skripta ih kopira ovde (a
`--link` pravi simboličke veze, za rad na Pro-u):

| Putanja | Šta je | Bez Pro-a (Free) |
| --- | --- | --- |
| `src-tauri/src/pro/` | Rust modul: komande i logika Pro funkcija, licenca | `src-tauri/src/pro_stub.rs` |
| `crates/celik-sign/` | PKCS#11, XML potpis, Smartbox | ostaje samo `Cargo.toml` i prazan `src/lib.rs` (posle seljenja) |
| `ui/src/lib/pro/` | Svelte stranice i paneli, Pro API, Pro prevodi | `ui/src/lib/pro-stub/` |

`src-tauri/providers/fines.json` (i potpis) **ostaje u ovom repou**: instalirane verzije 0.2
do 0.4 ga preuzimaju sa `raw.githubusercontent.com/vahidzekic/Chitach/main/...`.

## Kako se ubacuje

**Rust.** Feature `pro` crate-a `chitach` uključuje modul `src/pro/` i njegove zavisnosti
(`celik-sign`, `ring`, `sha1`, `base64`, `x509-parser`). Bez njega `src/pro_stub.rs` stoji na
istom mestu sa istim funkcijama: `setup`, `allows`, `require`, `start_mode`, `on_exit`,
`COMMANDS` i `invoke_handler`. Ostatak aplikacije zove samo njih (`crate::pro::...`), a
spisak plaćenih funkcija je u `src/features.rs`. Komande se biraju po imenu: Pro komande idu
Pro handler-u, ostale besplatnom (`lib.rs`). `get_app_info` vraća `pro: true|false`.

Dok je Pro kod još ovde, `pro` je podrazumevan (`default = ["pro"]`). Posle seljenja
podrazumevano je Free, a zvanični build dodaje `--features pro`.

**UI.** Aplikacija uvozi Pro modul kao `$pro` (`vite.config.ts`, `tsconfig.app.json`): to je
`src/lib/pro` ako postoji, inače `src/lib/pro-stub`. `CHITACH_PRO=0` uvek bira Free. Šta
Pro dodaje opisuje `src/lib/pro-api.ts` (stranice, paneli ispod saobraćajne, delovi
podešavanja, prevodi, učitavanje licence). `proAvailable()` u `stores/app.svelte.ts` je
`false` u Free buildu i tada se ne prikazuje ništa o Pro-u.

## Komande

```sh
# Free (i u CI-ju): bez Pro modula
cargo clippy -p chitach --all-targets --no-default-features -- -D warnings
CHITACH_PRO=0 npm --prefix ui run build

# Pro: privatni repo u ./pro, ../chitach-pro ili $CHITACH_PRO_DIR
scripts/with-pro.sh npm --prefix ui run tauri -- build --features pro
scripts/with-pro.sh --link true   # za rad na Pro kodu
```

CI (`ci.yml`) proverava obe varijante: pun build i Free (UI bez `src/lib/pro`, Rust bez
feature-a `pro`).

## Sledeći koraci (iz plana)

1. Spojiti PR #21 (iOS build na GitHub-u).
2. ✓ Priključak u javnom repou (ovaj dokument).
3. Preneti Pro putanje sa istorijom u `vahidzekic/chitach-pro`, dodati njegov CI (Linux).
4. Deploy ključ (samo čitanje) na privatni repo, secret `PRO_DEPLOY_KEY` u ovom repou.
5. Obrisati Pro putanje odavde (i dodati ih u `.gitignore`), `default` feature bez `pro`,
   a `release.yml`, `android.yml` i `ios.yml` preuzimaju privatni repo i grade sa
   `scripts/with-pro.sh ... --features pro`.
6. Lokalni buildovi na Mac-u: `chitach-pro` kloniran pored `Chitach` u `~/Projects/`.
7. README, CONTRIBUTING i sajt.
8. Izdanje v0.5.0.

Slike: [29-free-vehicle.png](screenshots/29-free-vehicle.png) (saobraćajna u Free buildu, bez
kazni, istorije i JSON/Excel dugmadi) i [30-free-settings.png](screenshots/30-free-settings.png).
