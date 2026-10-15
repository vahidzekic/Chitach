# Istraživanje: istorija vozila (prva registracija, udesi, kilometraža)

> Status: oktobar 2026. **Faze 1 i 2 su implementirane, faza 3 je pripremljena** (v. §13 „Šta je
> implementirano" i §14 „Dopisi"); slanje dopisa i dogovori sa UOS-om/ABS-om/servisima nisu urađeni.
> Cilj: kada se ubaci kartica saobraćajne dozvole, Chitach pomaže korisniku da sazna istoriju
> vozila po uzoru na carVertical: datum prve registracije, udese i štete, originalnu (stvarnu)
> kilometražu, broj vlasnika, krađu, uvoz — što potpuniju istoriju.

## 0. Metodologija i ograničenja (pročitati prvo)

- Kao i za [07-istrazivanje-provera-kazni.md](07-istrazivanje-provera-kazni.md): **nijedna
  stranica nije učitana uživo.** `WebFetch` vraća `getaddrinfo ENOTFOUND`, a `curl` dobija
  `CONNECT tunnel failed, response 403` od izlaznog proxy-ja (politika okruženja). Korišćena je
  samo web pretraga (sažeci i isečci stranica).
- Oznake pouzdanosti su iste kao u dokumentu 07:
  - **[Z]** – sažetak/isečak **zvanične** stranice (nije učitana uživo);
  - **[K]** – javni kod / dokumentacija biblioteke;
  - **[T]** – treće strane (mediji, agregatori, konkurenti);
  - **[P]** – pretpostavka autora, **mora se proveriti**.
- Cene se menjaju i izvori se ne slažu; sve cene su orijentacione i navedene sa izvorom.

## 1. Kratak odgovor

1. **Ne postoji jedan izvor koji daje „celu istoriju".** carVertical i slični servisi spajaju
   stotine izvora (registri, osiguravači, servisi, oglasi, aukcije) i naplaćuju izveštaj
   ~25–37 € **[T]**. Nijedan nema javan, samouslužan API; B2B pristup je po ugovoru **[Z/T]**.
2. **Za vozila u Srbiji najvredniji domaći izvor je UOS „CarCheck"** (Udruženje osiguravača
   Srbije): besplatno godina proizvodnje, prva registracija u Srbiji i status registracije;
   za 890 RSD (SMS) **broj šteta iz autoodgovornosti, promena boje, broj vlasnika, taksi/lizing
   /pravno lice** **[Z/T]**. Samo mobilna aplikacija, bez API-ja.
3. **Kilometraža u Srbiji danas nema javni registar.** CISTEP (centralni sistem tehničkih
   pregleda) je u probnom radu do 15. 12. 2026, puna primena od 1. 1. 2027 **[T]**; da li će
   kilometraža biti javno dostupna — nije najavljeno.
4. **Prva registracija i godina proizvodnje već su na kartici** (`date_of_first_registration`,
   `year_of_production`), a VIN omogućava **besplatnu lokalnu proveru** (proizvođač, zemlja,
   provera usklađenosti sa podacima na kartici).
5. **Preporuka:** faza 1 bez troškova i bez servera — panel „Istorija vozila" sa podacima sa
   kartice, lokalnim VIN dekodiranjem i linkovima (UOS CarCheck, carVertical, autoDNA…, po
   mogućstvu sa VIN-om u URL-u / partnerskim linkom). Faza 2 — automatski izveštaj preko B2B API-ja
   **sa ključem korisnika** (firme), jer desktop aplikacija bez servera ne može bezbedno da
   čuva naš API ključ ni da naplaćuje. Detalji u §6–§9.

## 2. Šta imamo sa kartice

Iz `VehicleDocument` (`crates/celik-core/src/document.rs`):

| Polje | Upotreba za istoriju |
|---|---|
| `vehicle_id_number` (VIN, polje E) | **Ključ za sve komercijalne servise** i za lokalno dekodiranje |
| `date_of_first_registration` (polje B) | Prva registracija — **već imamo**. Da li je to prva registracija bilo gde ili prva u Srbiji (za uvezena vozila) — **proveriti na primerima [P]** |
| `year_of_production` | Poređenje sa godinom iz VIN-a (10. znak) i sa prvom registracijom |
| `vehicle_make`, `commercial_description`, `vehicle_type` | Poređenje sa proizvođačem iz VIN-a (WMI) |
| `engine_id_number`, `engine_capacity`, `type_of_fuel`, `maximum_net_power` | Poređenje sa specifikacijom iz VIN dekodera (zamena motora, „prepravke") |
| `colour_of_vehicle` | Poređenje sa „promenom boje" iz UOS izveštaja |
| `registration_number_of_vehicle` | Ključ za UOS CarCheck i domaće izvore |
| `owner_*`, `users_*` | **Ne šalje se nikome** — istorija vozila ne zahteva podatke o osobi |

**Kilometraža nije na kartici** i ne postoji na saobraćajnoj dozvoli.

## 3. Domaći izvori (Srbija)

### 3.1 UOS „Proveri vozilo – CarCheck" (Udruženje osiguravača Srbije)

- Pokrenuta oktobra 2023 **[Z: uos.rs/2023/10/20/uos-pokrenuo-aplikaciju-carcheck…]**; Android
  `net.normasoft.uos.carcheck`, iOS `id6466516824` **[Z]** (razvija Normasoft **[P]** iz imena paketa).
- **Besplatno**: da li je vozilo u bazi, godina proizvodnje, datum prve registracije u Srbiji
  (za vozila prvi put registrovana posle 1. 6. 2011) i da li je i do kada registrovano **[T: 021.rs, Tanjug]**.
- **Plaćeno (890 RSD, zahtev SMS-om iz aplikacije)**: **broj šteta** prijavljenih po osnovu
  autoodgovornosti u kojima je vozilo učestvovalo, da li je menjana boja, da li je bilo u
  vlasništvu pravnog lica / lizing kuće ili korišćeno kao taksi, **broj vlasnika** **[T]**.
  Cena je iz 2023 — proveriti aktuelnu.
- Ograničenje: 4 provere dnevno po uređaju za registrovane korisnike, 2 za ostale **[T]**.
- Ključ upita: verovatno registarska oznaka ili broj šasije **[P — proveriti u aplikaciji]**.
- API / web verzija: **nije pronađena**. Aplikacija je „nekomercijalna", naplata je simbolična
  zbog troškova infrastrukture **[T]** → razumno je pisano pitati UOS za partnerski pristup.
- Ograničenje sadržaja: samo štete iz **domaće autoodgovornosti** (ne kasko, ne štete u
  inostranstvu, ne nepoprijavljene popravke) **[P]**.

### 3.2 MUP / eUprava

- **Javna provera istorije vozila po VIN-u ili tablici ne postoji** **[T]**. eUprava ima usluge
  MUP-a vezane za **osobu** (saobraćajni prekršaji za 4 godine, izjava ko je upravljao vozilom)
  **[T: N1]**, ne „istoriju vozila".
- Prenos vlasništva i registracija idu preko MUP-a; podaci o prethodnim vlasnicima su lični
  podaci i nisu javni **[P]**.
- **Provera da li je vozilo ukradeno**: javni servis MUP-a/Interpola (SMV baza) **nije pronađen**
  — Interpol SMV je dostupan samo policiji **[P]**. Komercijalni servisi tvrde da proveravaju
  policijske baze više zemalja **[T: carVertical]**.

### 3.3 ABS i CISTEP (tehnički pregledi)

- **CISTEP** — centralni informacioni sistem tehničkih pregleda: probni rad 15. 6 – 15. 12. 2026,
  puna primena planirana od 1. 1. 2027 **[T: tehnicki-pregled.rs, mondo.rs, infoera.rs]**.
- Čuva podatke o stanicama, kontrolorima, strankama, vozilima, **merenjima**, ocenama i
  nedostacima, šalje ih u realnom vremenu MUP-u i ABS-u, bez naknadne izmene **[T]**. Veza sa
  eUpravom je najavljena **[T: Informer]**.
- **Kilometraža**: nijedan izvor ne potvrđuje da se beleži niti da će biti javno dostupna **[T]**.
  Ako CISTEP počne da beleži stanje putomera (kao poljski CEPiK ili holandski RDW), to bi postao
  **jedini zvanični izvor kilometraže** za vozila u Srbiji → **pratiti** i pitati ABS.
- ABS vodi i bazu tehničkih karakteristika vozila (homologacija) **[T: izmene ZBSP]** — nije
  javna kao servis.

### 3.4 Ostalo

- Servisne knjižice / ovlašćeni servisi: istorija održavanja postoji kod proizvođača (OEM
  portali za dilere), nije dostupna trećim licima bez ugovora **[P]**. Vincario prodaje „OEM
  VIN lookup" i „Service history" za neke marke **[T: TrustRadius]**.
- Oglasi (polovniautomobili.com i sl.): carVertical objavljuje statistike za Srbiju (npr. 38,5 %
  proverenih vozila sa istorijom oštećenja; ~5,5 % popularnih modela sa vraćenom kilometražom;
  28 % polovnjaka iz Italije) **[T: carVertical, N1]**. Zvanična saradnja sa polovniautomobili.com
  nije pronađena **[T]**.

## 4. Komercijalni servisi (VIN → izveštaj)

| Servis | Pokrivenost | Šta daje | Cena (potrošač) | API / B2B | Pouzdanost |
|---|---|---|---|---|---|
| **carVertical** (LT) | ~30 zemalja, „1000+ baza", ima `/rs` verziju | štete, vraćena kilometraža, krađa, slike, specifikacija, tržišna cena, broj zemalja/vlasnika | ~30–37 € pojedinačno, paketi 10/30/100 za firme | **API postoji** (izveštaj kao web/PDF, VIN dekoder, besplatno „US-origin" upozorenje), **po kontaktu**, javne dokumentacije nema; **affiliate program** (90 dana kolačić, isplata od 50 €) | [Z/T] |
| **autoDNA** (PL) | 26+ tržišta EU + SAD/Kanada | krađa, štete, servis, kilometraža, vlasnici | 24,99 € pojedinačno | B2B paketi; javni API nije pronađen | [Z/T] |
| **Carfax Europe** | 21+ zemalja, Nemačka od 1. 7. 2026 | istorija vozila, kilometraža, štete | — | B2B za dilere/osiguravače (reseller); javni API nije pronađen | [T] |
| **Cebia AUTOTRACER** (CZ) | CZ, SK + partneri iz EU | kilometraža, štete, taksi, tržišna cena | — | besplatno za sertifikovane prodavce; API nije pronađen | [Z/T] |
| **Vincario** (bivši vindecoder.eu, CZ) | globalno | **VIN dekoder**, krađa, tržišna vrednost, OEM podaci, servisna istorija (neke marke) | — | **javni samouslužni API**: ~0,22–0,49 $ dekodiranje, ~0,90–1,99 $ krađa/vrednost, 20 besplatnih upita, 60 upita/min | [Z/T], cene se razlikuju po izvorima |
| **VinAudit, Carfax US, AutoCheck** | SAD | NMVTIS, aukcije | ~10–40 $ | API postoji | [T] — korisno samo za vozila uvezena iz SAD |

Zaključci:
- Za „pravu" istoriju (udesi u inostranstvu, kilometraža pre uvoza) **komercijalni servis je
  jedina opcija**; većina polovnih vozila u Srbiji je uvezena **[T]**, a domaći izvori vide samo
  period u Srbiji.
- Jedini sa **samouslužnim** API-jem bez pregovora je **Vincario** (dekodiranje + krađa +
  vrednost, ali ne puna istorija šteta/kilometraže za EU). carVertical i autoDNA zahtevaju ugovor.
- carVertical ima srpsku lokalizaciju i affiliate program → link iz aplikacije je izvodljiv odmah.

## 5. Zvanični registri u EU (za vozila iz uvoza)

| Zemlja | Servis | Ključ upita | Kilometraža | Pristup | Upotrebljivo za Chitach? |
|---|---|---|---|---|---|
| Holandija | RDW Open Data (`opendata.rdw.nl`, skup `m9d7-ebf2`), NAP / „tellerstandoordeel" | holandska tablica (kenteken) | ocena logičnosti putomera (od 2014; ranije NAP) | **besplatan otvoren API, CC0** | Samo ako je poznata stara NL tablica; po VIN-u javni skup verovatno ne pretražuje **[P]** |
| Poljska | `historiapojazdu.gov.pl` (CEPiK) | tablica + VIN + datum prve registracije | stanja sa tehničkih pregleda od 2014 | web forma, API nije pronađen | Samo uz staru PL tablicu i datum |
| UK | DVSA MOT History API | UK tablica | **stanje putomera na svakom MOT-u** | API ključ za odobrene organizacije | Retko (vozila sa desnim volanom) |
| Belgija | Car-Pass | kod sa Car-Pass dokumenta | potpuna istorija | dokument pri prodaji | Ako kupac dobije Car-Pass od prodavca |
| Francuska | HistoVec | podaci sa francuske saobraćajne | delimično | web, uz podatke vlasnika | Ne |
| Italija, Nemačka | — | — | **nema javnog registra kilometraže** **[T: paperjam.lu]** | — | Ne (a to su glavna izvorna tržišta) |

- **EU „Roadworthiness package" (2025/0097(COD), 2025/0096(COD))**: predlog da servisi beleže
  kilometražu, proizvođači šalju stanje putomera povezanih vozila, a države razmenjuju istoriju
  kilometraže (najmanje 3 godine) preko MOVE-HUB / EUCARIS **[Z: Savet EU, EP]**. Još nije
  usvojeno; pristup je predviđen za organe, ne za potrošače **[T]**.
- Zaključak: nacionalni registri su korisni samo ako se zna **strana tablica**, koju saobraćajna
  dozvola ne sadrži. Komercijalni agregatori upravo to rešavaju (VIN → strane tablice → registri).

## 6. VIN dekodiranje (besplatno, lokalno)

- Struktura (ISO 3779): **WMI** (znakovi 1–3: zemlja i proizvođač), **VDS** (4–9: model,
  karoserija, motor — specifično za proizvođača), **VIS** (10–17: 10. znak godina modela, 11.
  fabrika, ostalo serijski broj). Kontrolna cifra (9. znak) i godina iz 10. znaka su
  **obavezni samo za Severnu Ameriku**; evropski proizvođači ih često ne koriste **[P]**.
- **NHTSA vPIC** (`vpic.nhtsa.dot.gov/api/vehicles/DecodeVinValues/{VIN}?format=json`):
  besplatno, bez ključa, ali **slaba pokrivenost evropskih modela** koji se nisu prodavali u SAD **[Z/T]**.
- Rust biblioteke: **`vin-decode`** (0BSD, ugrađene tabele iz vPIC-a, radi bez mreže) i
  **`mb_vin`** (ISC, parsiranje/validacija po ISO 3779, WMI → proizvođač/zemlja; nestabilna verzija)
  **[K: docs.rs, lib.rs]** — izvor WMI tabele u `mb_vin` treba proveriti.
- Komercijalno za EU: Vincario (§4).

Šta Chitach može lokalno i besplatno, bez slanja podataka:
1. validacija formata VIN-a (17 znakova, bez I/O/Q);
2. WMI → proizvođač i zemlja proizvodnje;
3. **provere usklađenosti** sa karticom: proizvođač iz WMI vs `vehicle_make`; godina iz 10. znaka
   (ako je proizvođač koristi) vs `year_of_production`; `year_of_production` vs
   `date_of_first_registration` (npr. prva registracija 3+ godine posle proizvodnje → verovatno
   uvoz polovnog vozila);
4. starost vozila i procena „očekivane" kilometraže (npr. 15.000–20.000 km/godišnje, samo kao
   informativni raspon, jasno označen) **[P]**.

## 7. Pravni i privatnosni aspekti

1. **VIN** sam po sebi opisuje stvar, ali na saobraćajnoj je povezan sa vlasnikom, pa ga treba
   tretirati kao podatak o ličnosti kada se šalje dalje (isti stav Poverenika kao za tablicu,
   v. dokument 07 §3) **[Z/P]**.
2. Spoljnim servisima slati **samo VIN** (eventualno tablicu za UOS), **nikad** ime, JMBG ili adresu.
3. Linkovi ka komercijalnim servisima (carVertical, autoDNA) šalju VIN **trećoj strani** — za
   razliku od kazni (gde su portali zvanični). Potrebno je jasno obaveštenje i klik korisnika;
   bez automatskog slanja.
4. Za firme (auto-placevi, agencije za registraciju) Chitach je alat, a firma je rukovalac
   podataka (ZZPL) — isto upozorenje kao u panelu kazni.
5. Affiliate prihod: ako se koristi partnerski link, to mora biti **vidljivo** korisniku
   („partnerski link") — open-source projekat ne sme da skriva komercijalni odnos.
6. Uslovi korišćenja servisa i `robots.txt` **nisu mogli biti pročitani**; scraping
   komercijalnih izveštaja ili UOS aplikacije **isključen** — samo zvanični API ili link.

## 8. Pristupi implementaciji

| Pristup | Opis | Prednosti | Mane |
|---|---|---|---|
| **(a) Lokalno** | VIN dekoder + provere usklađenosti + podaci sa kartice | besplatno, bez mreže, bez privatnosnih dilema | ne daje udese ni kilometražu |
| **(b) Link-out** | dugmad „Proveri na carVertical / autoDNA / UOS CarCheck" sa VIN-om u URL-u (ako je podržano) ili u clipboard-u | radi odmah, nema API ključa, nema troška; moguć affiliate prihod | korisnik plaća i čita izveštaj van aplikacije |
| **(c) B2B API sa ključem korisnika (BYOK)** | firma unese svoj carVertical / autoDNA / Vincario API ključ u podešavanja; Chitach direktno zove API i prikazuje izveštaj | pravi izveštaj u aplikaciji; nema našeg servera ni naplate; ključ ostaje kod korisnika | samo za firme sa ugovorom; različiti formati odgovora po servisu |
| **(d) Sopstveni posrednički server** | naš server čuva API ključ, naplaćuje izveštaj (kartica / kredit), prosleđuje VIN | „jedan klik" za sve korisnike | potreban backend, naplata, ugovor sa servisom, ZZPL obaveze, održavanje — velika promena za lokalnu open-source aplikaciju |
| **(e) Partnerstvo sa UOS-om** | API za CarCheck podatke (štete u Srbiji, broj vlasnika) | najbolji domaći podaci | zavisi od UOS-a; verovatno spor proces |

Napomena: API ključ ugrađen u desktop binarku je **javan** (lako se izvuče) — zato (c) ili (d),
nikad ključ u aplikaciji.

## 9. Preporuka i plan po fazama

**Preporuka: (a) + (b) odmah, (c) kao opcija za firme, (e) pokrenuti paralelno; (d) ne raditi
dok ne postoji jasna potreba i model naplate.**

**Faza 1 — panel „Istorija vozila" (bez troškova, oko 1–2 PR-a)**
1. Novi modul `src-tauri/src/history.rs`: `normalize_vin`, validacija, WMI dekodiranje
   (crate `vin-decode` ili sopstvena mala WMI tabela u `providers/wmi.json` za proizvođače
   prisutne u Srbiji — odluka posle provere veličine i pokrivenosti crate-a).
2. Provere usklađenosti (§6) kao lista „✓ / ⚠ / ?" uz objašnjenje; nikad tvrdnja „vozilo je
   ispravno" — samo „podaci su usklađeni / nisu usklađeni".
3. Definicije provajdera u `src-tauri/providers/history.json`, **isti format i loader kao
   `fines.json`** (`kind: "link"`, novi placeholder `{vin}` pored `{plate}`, allowlist domena,
   samo `https`). Početni izvori: UOS CarCheck (linkovi ka App Store / Google Play + uputstvo
   „unesite tablicu/VIN"), carVertical (`/rs`), autoDNA, Carfax Europe, a za vozila iz SAD
   VinAudit/Carfax.
4. UI: `ui/src/lib/pages/document/HistoryPanel.svelte` pored „Provera kazni"; dugmad
   „Kopiraj VIN", „Otvori ↗"; oznaka „plaćeni servis treće strane" i, ako se uključi, „partnerski link".
5. Komande `history_info(doc)` (lokalno) i ponovo korišćenje `tauri-plugin-opener`; mock u
   `CHITACH_MOCK=1`.
6. Screenshot u `docs/screenshots/`, ažurirati `04-api-ugovor.md` i `05-korisnicko-uputstvo.md`.

**Faza 2 — BYOK izveštaj za firme (opt-in)**
1. Podešavanja: `history: { provider, apiKey, consentGiven }`; ključ u sistemskom keychain-u
   (crate `keyring`), ne u `settings.json`.
2. Tip provajdera `api` u `history.json` sa parserom po servisu (`carvertical`, `vincario`…);
   prvi kandidat **Vincario** (jedini sa javnom dokumentacijom i samouslužnim ključem), zatim
   carVertical kada se dobije dokumentacija od B2B tima.
3. Saglasnost pre prvog slanja VIN-a („VIN se šalje servisu X; servis naplaćuje izveštaj po vašem
   ugovoru"); rezultat samo u memoriji (kao keš kazni), izvoz u PDF opcionalno.
4. Normalizovan model rezultata:
   ```rust
   struct HistoryReport {
       provider: String,
       first_registration: Option<String>,
       countries: Vec<String>,
       odometer: Vec<OdometerReading>,   // datum, km, izvor
       damages: Vec<DamageRecord>,       // datum, zemlja, iznos, opis
       stolen: Option<bool>,
       owners: Option<u32>,
       rollback_suspected: Option<bool>,
       report_url: Option<String>,       // pun izveštaj kod servisa
   }
   ```

**Faza 3 — zvanični domaći podaci**
1. Pisani zahtev UOS-u za API/partnerski pristup CarCheck podacima (štete, vlasnici, boja, taksi).
2. Pratiti CISTEP / eUpravu od 1. 1. 2027 — ako kilometraža sa tehničkih pregleda postane
   dostupna (javni servis ili otvoreni podaci), dodati kao zvanični izvor kilometraže.
3. Pratiti EU Roadworthiness package (MOVE-HUB) zbog razmene kilometraže za uvezena vozila.

**Ne preporučuje se:** scraping carVertical/UOS aplikacije, API ključ u binarki, sopstveni
server za naplatu u ovoj fazi, automatsko slanje VIN-a bez klika korisnika.

## 10. UX predlog

```
┌ Istorija vozila — VW Golf, VIN WVWZZZ1KZ…123 ───────────────────────────┐
│ Sa kartice                                                               │
│  Godina proizvodnje 2015 · Prva registracija 12.03.2019 · Starost 11 god │
│  ⚠ Prva registracija 4 godine posle proizvodnje — verovatno uvezeno polovno│
│ VIN provera (lokalno, ništa se ne šalje)                                 │
│  ✓ Proizvođač iz VIN-a: Volkswagen (Nemačka) — odgovara kartici          │
│  ? Godina iz VIN-a: proizvođač ne koristi 10. znak                       │
│ Domaća istorija                                                          │
│  ● UOS CarCheck — štete iz autoodgovornosti, broj vlasnika (890 RSD)  [Aplikacija ↗]│
│ Istorija iz inostranstva (plaćeni servisi trećih strana)                 │
│  ● carVertical  ~30 €   [Kopiraj VIN] [Otvori ↗]                          │
│  ● autoDNA      ~25 €   [Kopiraj VIN] [Otvori ↗]                          │
│ ⓘ Kilometraža se u Srbiji ne vodi u javnom registru.                     │
└──────────────────────────────────────────────────────────────────────────┘
```

## 11. Otvorena pitanja (ručna provera pre implementacije)

1. Da li je polje B na srpskoj saobraćajnoj prva registracija bilo gde ili prva u Srbiji?
   (Proveriti na kartici uvezenog vozila.)
2. UOS CarCheck: ključ upita (tablica/VIN), aktuelna cena, da li postoji web verzija ili API; kontakt UOS.
3. carVertical: da li URL prihvata VIN (npr. `?vin=`), uslovi affiliate programa za open-source
   aplikaciju, dokumentacija B2B API-ja i cena po izveštaju.
4. autoDNA / Carfax Europe: postojanje API-ja i partnerskih linkova.
5. Vincario: aktuelan cenovnik, pokrivenost evropskih modela, oblik odgovora.
6. `vin-decode` / `mb_vin`: veličina binarke, pokrivenost evropskih WMI kodova, licence.
7. CISTEP: da li se beleži stanje putomera i hoće li biti dostupno građanima (pitati ABS).

## 12. Izvori

Zvanični (viđeni kroz pretragu, ne učitani uživo):
- https://uos.rs/2023/10/20/uos-pokrenuo-aplikaciju-carcheck-za-laksu-kupovinu-i-prodaju-vozila-lat/
- https://play.google.com/store/apps/details?id=net.normasoft.uos.carcheck , https://apps.apple.com/us/app/proveri-vozilo-carcheck/id6466516824
- https://www.carvertical.com/rs , https://www.carvertical.com/business/api , https://www.carvertical.com/affiliate-program
- https://www.carvertical.com/rs/help/za-poslovne-klijente/sta-su-paketi-izvestaja-carvertical
- https://www.autodna.com/ , https://www.autodna.com/blog/?p=4787
- https://www.carfax.eu/press/carfax-launches-in-germany-more-transparency-and-safety-when-buying-a-used-car
- https://www.cebia.cz/pruvodce/tachometr
- https://vincario.com/api-docs/ , https://vincario.com/blog/?p=698
- https://vpic.nhtsa.dot.gov/api/
- https://opendata.rdw.nl/api/views/jqs4-4kvw , https://data.overheid.nl/en/dataset/15920-open-data-rdw--tellerstandoordeel-trend-toelichting
- https://findtransportdata.dft.gov.uk/dataset/mot-history-api
- https://data.consilium.europa.eu/doc/document/ST-15613-2025-INIT/en/pdf , https://www.europarl.europa.eu/RegData/etudes/BRIE/2026/782608/EPRS_BRI(2026)782608_EN.pdf

Biblioteke:
- https://docs.rs/vin-decode , https://lib.rs/crates/mb_vin

Treće strane:
- https://www.021.rs/story/Info/Biznis-i-ekonomija/356043/Udruzenje-osiguravaca-Pokrenuta-aplikacija-CarCheck-za-pomoc-pri-kupovini-i-prodaji-polovnih-vozila.html
- https://www.tanjug.rs/srbija/drustvo/55442/pokrenuta-aplikacija-carcheck-za-laksu-kupovinu-i-prodaju-polovnih-vozila/vest
- https://tehnicki-pregled.rs/cistep-sistem-tehnicki-pregled-srbija/ , https://mondo.rs/Automobili/Vesti/a2280969/menja-se-cena-registracije-novi-sistem-na-tehnickim-pregledima.html , https://infoera.rs/2026/05/11/novi-digitalni-sistem-tehnickih-pregleda-sta-se-menja-za-vozace/
- https://n1info.rs/vesti/dve-nove-usluge-mup-a-na-portalu-euprava/
- https://n1info.rs/magazin/auto/izvestaji-o-istoriji-vozila-otkrivaju-surovu-stvarnost-trzista-polovnjaka/ , https://n1info.rs/magazin/auto/odakle-se-uvozi-najvise-polovnjaka-u-srbiju-ne-nije-nemacka/
- https://www.fahrzeugschein.de/blog/artikel/carvertical-erfahrungen
- https://www.trustradius.com/compare-products/vincario
- https://rankomat.pl/samochod/jak-sprawdzic-historie-pojazdu
- https://paperjam.lu/article/bientot-fini-le-bricolage-du-kilometrage-des-voitures-doccasion
- https://www.eccnet.eu/publication/mileage-verification-car-pass-ensuring-accuracy-vehicles-abroad

## 13. Šta je implementirano

| Stavka | Gde | Napomena |
|---|---|---|
| Tabela WMI regiona (ISO 3780) i ~100 proizvođača čestih u Srbiji | `src-tauri/providers/wmi.json` | zemlja kao ISO 3166-1 alpha-2 (UI je prevodi preko `Intl.DisplayNames`); `modelYear: true` samo za proizvođače koji upisuju godinu u 10. znak (grupa VW, Volvo, Hyundai/Kia, Tesla, Severna Amerika); `country` za fabrike van WMI regiona (Tesla Berlin `XP7`) |
| Lokalna provera VIN-a | `src-tauri/src/history.rs` (`normalize_vin`, `vin_format`, `decode`, `check_digit`, `model_year`, `checks`) | oblik VIN-a, proizvođač i zemlja, godina modela (30-godišnji ciklus, najbliži tekućoj godini), kontrolna cifra samo za Severnu Ameriku, provere usklađenosti: marka (`names` u `wmi.json`, poređenje bez dijakritika i ćirilice), godina modela ±1, prva registracija pre proizvodnje / 2+ godine posle, starost. Ništa se ne šalje. |
| Definicije servisa | `src-tauri/providers/history.json` | UOS CarCheck (Android + iOS, ključ tablica), carVertical, autoDNA, CARFAX Europe, Cebia, NHTSA opozivi (`{vin}` u URL-u), VinAudit, Vincario (`kind: "api"`). Samo `https`; nevažeći ili nepoznati unosi se ignorišu. `partner: true` → UI prikazuje „partnerski link" (trenutno nijedan). |
| Komande | `src-tauri/src/commands.rs` | `history_info`, `history_set_api_key`, `history_remove_api_key`, `history_api_check` (v. `docs/04-api-ugovor.md`) |
| Panel „Istorija vozila" | `ui/src/lib/pages/document/HistoryPanel.svelte` | ispod „Provera kazni"; podaci sa kartice, provere, grupe servisa, „Kopiraj VIN", servisi za SAD se otvaraju sami za VIN iz Severne Amerike |
| Izveštaj preko API-ja sa ključem korisnika (faza 2) | `history.rs` (`api_url`, `vincario_control_sum`, `parse_vincario`, `report_with`, `HistoryCache`, `Secrets`) | Vincario v3.2: `{base}/{apiKey}/{controlSum}/{op}/{VIN}.json`, kontrolna suma = prvih 10 hex znakova SHA-1 od `VIN\|op\|apiKey\|secret` **[P — po dokumentaciji vindecoder.eu; potvrditi sa pravim ključem]**; operacije `decode`, `stolen-check`, `vehicle-market-value`. `decode` se čita kao lista `{label, value}`, ostale operacije se prikazuju „ravno" (`a.b`, `a[0].b`) bez tumačenja; `price`/`price_currency` se prikazuju kao naplaćeno. 401/403 ili greška o ključu → „ključ odbijen". Keš u memoriji 30 min (svaki upit se plaća), ponovni upit tek posle 60 s. URL sadrži ključ i nikad se ne loguje; u logu je samo WMI. |
| Čuvanje ključa | `secrets.json` pored `settings.json` | **odstupanje od §9**: umesto sistemskog keychain-a (crate `keyring` zahteva D-Bus/secret-service na Linuksu, dodatne sistemske zavisnosti) ključ je u posebnom fajlu sa dozvolama 0600 na Unix-u; UI nikad ne dobija ključ, samo `configured: true`. |
| Saglasnost i podešavanja | `settings.rs` (`history: { consentGiven }`), `SettingsPage.svelte` | dijalog pre prvog upita sa domenom; backend odbija upit bez saglasnosti ili ključa; opoziv i unos/uklanjanje ključa u podešavanjima |
| Mock | `ui/src/lib/api/mockHistory.ts`, `CHITACH_MOCK=1` | iste definicije; `?historyKey=1`, `?history=…` |
| Nedeljna provera linkova (faza 3) | `scripts/check-provider-links.mjs`, `.github/workflows/provider-links.yml` | ponedeljkom proverava stranice iz `fines.json` i `history.json` (nikad API/`check` adrese); 404/nepostojeći domen = crveni build, 401/403/429/503/timeout = upozorenje (zaštita od botova) |

Nije urađeno (zahteva odluku ili dogovor): carVertical/autoDNA API (nema javne dokumentacije —
v. §11.3), partnerski (affiliate) linkovi (potrebna registracija vlasnika projekta u programu),
podaci UOS CarCheck-a u aplikaciji (v. §14.1), kilometraža iz CISTEP-a (v. §14.2).

Dodavanje novog API servisa: novi unos u `history.json` sa `kind: "api"` i `api.service`, nova
vrednost `Service` i grana u `api_url`/`interpret` u `history.rs` (format zahteva i odgovora je
specifičan za servis).

## 14. Dopisi (faza 3) — nacrti za slanje

Nacrti su spremni za slanje sa adrese vlasnika projekta; Chitach ih ne šalje.

### 14.1 Udruženje osiguravača Srbije — pristup podacima CarCheck

> **Predmet:** Zahtev za partnerski pristup podacima aplikacije „Proveri vozilo – CarCheck"
>
> Poštovani,
>
> razvijam **Chitach**, besplatnu aplikaciju otvorenog koda za računare (Windows, macOS, Linux)
> koja čita srpske elektronske dokumente, među njima i saobraćajnu dozvolu
> (https://github.com/vahidzekic/Chitach). Kada korisnik ubaci saobraćajnu dozvolu, aplikacija
> mu pomaže da proveri vozilo — trenutno ga upućuje na vašu aplikaciju CarCheck.
>
> Želeli bismo da korisnicima omogućimo pristup istim podacima direktno iz Chitach-a
> (broj šteta iz autoodgovornosti, broj vlasnika, promena boje, taksi/lizing), uz iste uslove
> kao u aplikaciji: naplata po upitu, ograničenje broja upita po korisniku i bez čuvanja podataka
> na našoj strani. Chitach nema sopstveni server — upit bi išao direktno sa računara korisnika
> na vaš servis, uz izričitu saglasnost korisnika.
>
> Molim vas za informaciju:
> 1. da li postoji ili se planira API (ili web verzija) za CarCheck podatke;
> 2. pod kojim uslovima (ugovor, cena, tehnička dokumentacija, zaštita podataka) bi pristup bio moguć;
> 3. da li je dozvoljeno da Chitach otvara vašu aplikaciju/stranicu sa unapred popunjenom tablicom ili brojem šasije.
>
> Unapred hvala,
> Vahid Zekic, Chitach

### 14.2 Agencija za bezbednost saobraćaja — kilometraža u CISTEP-u

> **Predmet:** Upit o podacima o stanju putomera u sistemu CISTEP
>
> Poštovani,
>
> razvijam **Chitach**, besplatnu aplikaciju otvorenog koda za čitanje srpskih elektronskih
> dokumenata, uključujući saobraćajnu dozvolu (https://github.com/vahidzekic/Chitach).
> Kupci polovnih vozila nemaju javan način da provere kilometražu vozila u Srbiji, a vraćena
> kilometraža je čest problem.
>
> U vezi sa centralnim informacionim sistemom tehničkih pregleda (CISTEP), molim vas za informaciju:
> 1. da li se pri tehničkom pregledu u CISTEP beleži stanje putomera;
> 2. da li se planira da istorija stanja putomera (ili ocena njene logičnosti, kao holandski
>    RDW ili poljski CEPiK) bude dostupna vlasniku ili kupcu vozila, npr. preko eUprave ili kao
>    otvoreni podaci;
> 3. ako da — od kada i pod kojim uslovima bi aplikacije poput Chitach-a mogle da prikažu te podatke.
>
> Unapred hvala,
> Vahid Zekic, Chitach

### 14.3 carVertical — API i partnerski program

> **Subject:** API documentation and affiliate terms for an open-source desktop app
>
> Hello,
>
> I maintain **Chitach**, a free, open-source desktop app (Windows, macOS, Linux) that reads Serbian
> electronic documents, including the vehicle registration card
> (https://github.com/vahidzekic/Chitach). When a card is read, the app links to vehicle
> history services, including carVertical (Serbian version).
>
> Could you share:
> 1. the B2B API documentation and pricing (the app would call the API directly from the user's
>    computer with the business customer's own key; we run no server);
> 2. whether a link can open carVertical with the VIN pre-filled;
> 3. affiliate program terms for an open-source app (the link would be clearly marked as a partner link).
>
> Thank you,
> Vahid Zekic, Chitach
