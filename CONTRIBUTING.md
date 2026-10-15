# Doprinos projektu / Contributing

Hvala na interesovanju! Prijave grešaka, predlozi i pull request-ovi su dobrodošli.
*Contributions in English are equally welcome.*

## Prijava greške

Otvorite [issue](https://github.com/vahidzekic/Chitach/issues) i navedite:

* verziju Chitach-a i operativni sistem (sa arhitekturom: x64 / arm64 / Apple Silicon),
* model čitača kartica,
* tip kartice i **ATR** (prikazan u aplikaciji ili `chitach-cli --atr`),
* šta ste očekivali, a šta se desilo.

**Nikada ne šaljite lične podatke sa kartice** (JMBG, ime, adresu, fotografiju, PDF/JSON izvoz).

## Razvoj

Postavka okruženja, mock režim i build su opisani u [docs/06-razvoj.md](docs/06-razvoj.md).
Arhitektura: [docs/02-arhitektura.md](docs/02-arhitektura.md); ugovor komandi i događaja
između Rust-a i UI-ja: [docs/04-api-ugovor.md](docs/04-api-ugovor.md).

Pre slanja pull request-a:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix ui run check
npm --prefix ui run build
```

CI pokreće iste provere na svaki push i pull request.

## Smernice

* Kod, identifikatori i komentari su na engleskom; tekstovi za korisnika idu kroz prevode
  (`ui/src/lib/i18n`, `assets/translations`) — dodajte ključ za sve jezike.
* Fajlovi preneti iz Baš Čelika zadržavaju zaglavlje
  `Port of bas-celik (MIT, (c) Nikola Ubavić)`.
* Izmene API-ja između Rust-a i UI-ja prvo upišite u `docs/04-api-ugovor.md`.
* Za logiku čitanja kartica dodajte testove sa (anonimizovanim) primer podacima.
* Jedan pull request — jedna tema; opišite kako ste testirali (sa kojim čitačem/karticom).

## Licenca

Slanjem doprinosa pristajete da bude objavljen pod [MIT licencom](LICENSE) projekta.
