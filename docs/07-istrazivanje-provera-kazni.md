# Istraživanje: provera neplaćenih kazni za vozilo (saobraćajna dozvola)

> Status: istraživanje / predlog, oktobar 2026. **Faze 1 i 2 su implementirane** (v. §11 „Šta je
> implementirano"); faza 3 (preuzimanje potpisanih definicija, dodatni gradovi, zvanični API) nije.
> Cilj: kada se ubaci kartica saobraćajne dozvole, Chitach pomaže korisniku da proveri da li vozilo
> (tablica) ili vlasnik ima neplaćene parking kazne, saobraćajne kazne, putarinu i druga dugovanja.

## 0. Metodologija i ograničenja (pročitati prvo)

- **Direktno otvaranje sajtova nije bilo moguće iz okruženja u kome je rađeno istraživanje.**
  `WebFetch` je za *svaki* domen vraćao `getaddrinfo ENOTFOUND` (čak i za `en.wikipedia.org`), a
  `curl` je dobijao `CONNECT tunnel failed, response 403` od izlaznog proxy-ja (politika okruženja,
  ne odgovor ciljanih sajtova). Zato **nijedna stranica nije učitana uživo**, nije pregledan HTML,
  nisu presretnuti XHR pozivi i nisu pročitani `robots.txt` fajlovi.
- Korišćeni izvori: (1) web pretraga (sažeci i isečci stranica koje pretraživač vraća, uključujući
  zvanične stranice), (2) **javni izvorni kod na GitHub-u** (pretraga koda), u kome su drugi
  programeri već zapisali endpoint-e.
- Oznake pouzdanosti u dokumentu:
  - **[Z]** – potvrđeno sažetkom/isečkom **zvanične** stranice iz pretrage (stranica nije učitana uživo);
  - **[K]** – potvrđeno u **javnom kodu** na GitHub-u (tačan URL/parametar, ali ne nužno aktuelan);
  - **[T]** – navodi **trećih strana** (mediji, agregatori, blogovi);
  - **[P]** – pretpostavka / zaključak autora, **mora se proveriti** pre implementacije;
  - **[U]** – podatak koji je prijavio korisnik/saradnik, **nije provereno** iz ovog okruženja.
- Pre implementacije svakog izvora obavezno: otvoriti stranicu u pregledaču, DevTools → Network,
  zabeležiti tačan zahtev/odgovor, CAPTCHA, kolačiće, `robots.txt` i uslove korišćenja.

## 1. Šta imamo sa kartice

Iz `VehicleDocument` (`crates/celik-core/src/document.rs`) relevantno je:

| Polje | Primer | Upotreba za proveru |
|---|---|---|
| `registration_number_of_vehicle` | `BG123AB` | **Ključ za sve parking servise** (eDPK / ePPK / doplatne karte) |
| `vehicle_id_number` (VIN) | `VX123…` | Nijedan pronađeni javni servis ne proverava kazne po VIN-u |
| `owner_name`, `owners_surname_or_business_name`, `owners_personal_no` | ime, prezime, JMBG / matični broj firme | Registar neplaćenih kazni (RNK) traži ime + prezime + JMBG (fizičko lice) |
| `users_*` | korisnik vozila (ako se razlikuje od vlasnika) | isto kao vlasnik |

Zaključak: **parking dugovanja se vezuju za tablicu**, a **saobraćajni prekršaji (MUP) i neplaćene
kazne u registru vezuju se za osobu (JMBG)**, ne za vozilo. Javne provere „po tablici" za MUP kazne
**ne postoje** [T: dozvola.rs, akta.rs — „provera tuđih poena/kazni po tablici ne postoji"].

## 2. Nalazi po izvorima

### 2.1 JKP „Parking servis" Beograd — eDPK (elektronska dnevna parking karta)

- Zvanična provera po tablici: `https://www.parking-servis.co.rs/edpk` (i `/lat/edpk`, `/eng/edpk`) **[Z]**.
  Prikazuju se eDPK izdate u **poslednjih 60 dana** (broj naloga, zona, adresa, razlog, iznos sa
  popustom, rok); starija dugovanja se proveravaju preko **eŠaltera** („Zahtev za upit stanja") **[Z/T]**.
- Popust 50 % ako se plati u roku (zvanična engleska stranica pominje 20 dana; stariji mediji 8 dana — **proveriti**) **[Z/T]**.
- **Zaštita: Cloudflare Turnstile.** Agregator eparkiranje.com izričito navodi da je zvanična stranica
  za proveru zaštićena Cloudflare Turnstile proverom, da je zato ne upituje automatski i da je u
  kontaktu sa JKP „Parking servis" radi odobrenja API pristupa **[T]**. Ovo je ključni nalaz:
  direktni HTTP pozivi iz aplikacije **neće raditi** bez zaobilaženja zaštite (što ne radimo).
- Format URL-a: stariji javni scraper (2020) koristio je `https://parking-servis.co.rs/edpk/?fine=<TABLICA>`
  **[K: github.com/delicb/parking-service-tickets-scraper, `main.go`]**; korisničko uputstvo za
  eŠalter (PDF, 2026) navodno pominje oblik `…/edpk?plate=` **[T/P — samo iz sažetka pretrage]**.
  Ako parametar u URL-u i dalje popunjava polje, možemo otvoriti stranicu **unapred popunjenu**
  tablicom; Turnstile i dalje rešava korisnik **[P]**.
- Kanal za praćenje: zvanična mobilna aplikacija „Parking servis" šalje obaveštenja o novim eDPK i
  premeštenim vozilima za tablice upisane u aplikaciju **[Z/T]** — to je jedini zvanični „push" kanal.
- „Oko sokolovo" (vozilo sa kamerama Parking servisa) generiše eDPK; proverava se istom stranicom **[T: B92, una.rs]**.
- Postojeći neformalni servisi za Beograd: **kaznaservis.rs** (obaveštenja o eDPK; backend poziva
  sopstveni mikroservis `POST {SERVICE_IP}/api/plate/{plate}` — dakle sopstveni scraper, ne zvanični API
  **[K: github.com/dev-cyprium/kazna-servis-reimagined-be, `app/Services/ParkingservisService.php`]**),
  **jaoparking.com** (proverava 1–2 puta dnevno i šalje e-mail) **[T]**.
- Javni API / B2B: **nije pronađen**. Uslovi korišćenja sajta i `robots.txt` **nisu mogli biti pročitani**.

### 2.2 Novi Sad (JKP „Parking servis" Novi Sad) i ParkingNS portal

- Od 15. 1. 2024. nema papira na vetrobranu; posebna parking karta je elektronska (ePPK) i proverava
  se na sajtu po tablici **[T: 021.rs, nsuzivo.rs]**.
- Portal `https://portal.parkingns.rs/` („PortalNS") ima **gost** alate: provera izdate parking
  karte za vozilo i provera da li je vozilo na deponiji („pauk") **[Z]**.
- **Endpoint**: `GET https://portal.parkingns.rs/portal/auth/checkPPK?platePr=<TABLICA>` — vraća
  **JSON niz** (prazan niz = nema karata) **[K: github.com/s-kovacevic/ns-parking-ticket-checker,
  `main.go`, repo iz januara 2025]**. U tom kodu nema CAPTCHA tokena ni kolačića, što ukazuje da
  provera (bar tada) nije bila zaštićena **[P]**.
- Prema agregatoru eparkiranje.com, **isti ParkingNS sistem opslužuje još ~29 gradova/opština**
  (pominje se i Novi Pazar) **[T]** — zvanična lista nije pronađena. Ako je tačno, jedan endpoint
  pokriva veliki deo Srbije, ali **treba utvrditi** da li parametar grada postoji ili je baza zajednička.
- Format tablice: bez razmaka i crtica (npr. `NS123XY`) **[T]**.

### 2.3 Ostali gradovi

| Grad | Operater / portal | Provera po tablici? | Mehanizam | Pouzdanost |
|---|---|---|---|---|
| Niš | JKP „Parking servis" Niš, `nisparking.rs` (meni „еДПК ПРОВЕРА"); `nis.eparking.rs` | Da (eDPK) | nepoznat (forma) | [Z] postojanje, [T] detalji (60 dana, 8 dana rok) |
| Kragujevac | JKP „Šumadija", `jkpsumadija.rs/sr/info/sektor-parkinga/provera-opstih-dnevnih-parking-karti.html` | Da (opšte dnevne karte) | forma, unos tablice | [Z] |
| Čačak | JKP „Parking servis" Čačak, `status.parkingcacak.co.rs` | **Ne** — po **broju karte** | forma; odgovori „ПЛАЋЕН / СТОРНИРАН / НИЈЕ ПРОНАЂЕНА" | [Z] |
| Zrenjanin | JKP „Pijace i parkinzi", `zrenjanin.park4all.rs/surcharge` (ranije mojekazne.com) | Da (ePPK) | nepoznat | [T] |
| Zlatibor / Čajetina | KJP „Zlatibor", `edpk.kjpzlatibor.rs` | Da (eDPK) | nepoznat | [T] |
| Prokuplje | `prokuplje.mojekazne.com` (koncesionar) | Da | nepoznat | [T] |
| Novi Pazar | Parking servis Novi Pazar; `https://parkingservisnp.rs/info/` samo uključuje (iframe) **ParkingNS** stranicu `https://portal.parkingns.rs/ppkNP` | Da — polje „Registarska oznaka" (format `NPXXXXX`), dugme „Provera"; uputstvo za plaćanje samo za karte iz poslednjih godinu dana; podaci informativni | tačan API zahtev nije utvrđen (verovatno isti `checkPPK`, možda sa parametrom grada/operatera) | **[U]** prijavio saradnik, nije provereno iz ovog okruženja |
| ~28 drugih | navodno ParkingNS portal | Da | `checkPPK` | [T] |
| Subotica | JKP „Parking" Subotica | **Nije pronađena** online provera | SMS/kiosci | [T] |
| Pančevo | `pancevo.co.rs/parking` | **Nije pronađena** | SMS | [T] |
| Šabac, Kraljevo, Smederevo, Valjevo, Sombor | — | **Nema podataka** u pretrazi | — | — |

Nema nacionalnog registra parking kazni — svaki operater vodi svoju bazu **[T: eparkiranje.com, belgradecitycard.rs]**.

### 2.4 Ministarstvo pravde — Registar neplaćenih novčanih kazni (RNK)

- Javna pretraga: `https://rnk.sipres.sud.rs/` **[Z/T]**. Opcije „Fizičko lice", „Pravno lice",
  „Strano fizičko lice". Fizičko lice: **ime, prezime i JMBG**; pravno lice: **naziv i PIB** **[T]**.
  Bez prijave. Prazan rezultat: „Nema rezultata za zadate kriterijume pretrage!" **[T]**.
- Sadržaj: samo **pravnosnažne** neplaćene kazne i troškovi (prekršajni sud ili pravnosnažni
  prekršajni nalog) — ne i sveži nalozi ni kazneni poeni **[T]**. Upis ima posledice (npr. nemogućnost
  registracije vozila, zadržavanje na granici) **[T]**.
- Pravni osnov: Zakon o prekršajima čl. 331–336; Pravilnik o pristupu podacima iz jedinstvenog
  registra neplaćenih novčanih kazni (Sl. glasnik RS 13/2014) — organi pristupaju preko naloga
  koji odobrava ministar; podaci se koriste samo u svrhe Zakona **[T: paragraf.rs]**.
- CAPTCHA: **nije utvrđeno** (izvori se ne slažu / ne pominju). API: **nije pronađen**.
- Vezano za karticu: imamo ime, prezime i JMBG vlasnika (fizičko lice). Za firmu kartica nosi
  matični broj, a RNK traži PIB **[P]** → za pravna lica ne možemo popuniti automatski.

### 2.5 MUP / eUprava — saobraćajni prekršaji i kamere

- Usluga **„Pregled saobraćajnih prekršaja"** na `euprava.gov.rs/saobraćajni-prekršaji` **[Z]**:
  aktuelni i završeni prekršaji, iznos i rok, kazneni poeni, zabrana upravljanja, PDF naloga sa
  IPS QR kodom. Prijava: **ConsentID** ili **kvalifikovani elektronski sertifikat** **[Z/T]**.
  Prikazuje prekršaje **prijavljene osobe**, ne vozila.
- Kazne sa saobraćajnih kamera stižu poštom kao prekršajni nalog vlasniku; online provera po tablici
  **ne postoji** **[T]**.
- Javni API / otvoreni podaci (data.gov.rs): **nije pronađen** skup podataka o pojedinačnim kaznama;
  postoje samo statistike u saopštenjima **[T]**. Razmena podataka između organa ide preko
  državne infrastrukture (Servisna magistrala), nedostupne privatnim aplikacijama **[P]**.
- Sinergija sa Chitach-om: korisnik se na eUpravu može prijaviti **sertifikatom sa lične karte**
  (Chitach već radi sa PKCS#11) — ali to je prijava u pregledaču, ne nešto što Chitach treba da automatizuje.

### 2.6 Putarina (JP „Putevi Srbije", ENP/TAG)

- **Nema javne provere neplaćene putarine po tablici** **[T]**. Putevi Srbije čak upozoravaju na
  lažne SMS poruke o „nenaplaćenoj putarini" **[Z/T]**.
- TAG nalozi: `enp.amss.org.rs` (prijava e-mailom/lozinkom), Toll4All platforma; lista prolazaka i
  naplata vidi se samo u nalogu korisnika **[Z/T]**.
- Od 1. 7. 2026. kazne za vozila **4. kategorije** bez TAG-a; na obilaznici oko Beograda (MLFF,
  bez rampi) neregistrovani TAG dobija račun za naknadnu naplatu **[T: Forbes Srbija, plutonlogistics]**.
  Kako se tablica povezuje sa računom — **nije utvrđeno**.
- Zaključak: za putarinu samo link „Otvori ENP portal", eventualno samo za kategoriju vozila ≥ N2/N3.

### 2.7 Agregatori i aplikacije

| Servis | Šta radi | Izvor podataka | Napomena |
|---|---|---|---|
| **eparkiranje.com** | Besplatna provera 30+ (33/35/36 — brojke se razlikuju) gradova jednim unosom; B2B „flote" (svakodnevni e-mail izveštaji) | „upit ide zvaničnim portalima u realnom vremenu" (proxy); Beograd i Čačak samo link | Nastao iz teme na forumu Garaža (okt. 2025) — hobi/mali projekat; **nema javnog API-ja**; sam navodi da ne zaobilazi Turnstile [T] |
| **kaznaservis.rs** | Obaveštenja o eDPK (Beograd) | sopstveni scraper (`/api/plate/{plate}`) [K] | komercijalno |
| **jaoparking.com** | E-mail kad se pojavi eDPK (Beograd) | provera 1–2× dnevno [T] | operater nepoznat |
| Zvanična app „Parking servis" (BG) | plaćanje, obaveštenja o eDPK i premeštanju | zvanično | jedini zvanični push |
| ParkingNS app / portal | plaćanje, provera ePPK, deponija | zvanično | — |

Nijedan agregator ne nudi zvaničan, dokumentovan API, a nijedan operater nije objavio B2B API ponudu.

## 3. Pravni i privatnosni aspekti

1. **Zakon o zaštiti podataka o ličnosti (ZZPL, Sl. glasnik RS 87/2018)**:
   - Čl. 3: zakon se **ne primenjuje** na obradu koju fizičko lice vrši za lične potrebe / potrebe
     domaćinstva. Vlasnik koji proverava svoje vozilo — van domašaja zakona.
   - Ako Chitach koristi **firma** (rent-a-car, agencija za registraciju, tehnički pregled, auto-plac),
     ona je **rukovalac** i mora imati pravni osnov i informisati lica. Chitach treba to jasno da
     napiše u dijalogu saglasnosti.
   - Tablica sama po sebi Poverenik smatra podatkom o stvari, ali **postaje podatak o ličnosti kad se
     može povezati sa vlasnikom** [Z: poverenik.rs, saopštenje „Snimanje vozila u saobraćaju…"] —
     a na saobraćajnoj dozvoli upravo jeste povezana. JMBG je posebno osetljiv identifikator.
2. **Chitach je lokalna aplikacija bez sopstvenog servera**: podaci idu direktno sa računara korisnika
   na zvanični portal, isto kao kada korisnik sam ukuca tablicu. **Ne slati podatke trećim stranama**
   (agregatorima, sopstvenom serveru).
3. **Uslovi korišćenja / automatizovan pristup**: tekstovi uslova i `robots.txt` **nisu mogli biti
   pročitani**. Beograd aktivno blokira automatizaciju (Turnstile) → poštovati to. Za ParkingNS i
   ostale: pre uključivanja automatske provere **pisanim putem tražiti saglasnost** operatera
   (ili makar proveriti uslove) [P].
4. **RNK i JMBG trećeg lica**: Pravilnik je namenjen organima; javna pretraga je namenjena proveri
   **sopstvenog** statusa [T]. Chitach ne treba automatski da šalje JMBG; samo na izričit zahtev,
   uz upozorenje da se proverava lice sa kartice.
5. **Zakon o elektronskoj upravi / zloupotreba**: nikakvo zaobilaženje CAPTCHA, lažno predstavljanje
   klijenta ili masovni upiti. Ograničiti učestalost (jedan upit po ubacivanju kartice + ručno osvežavanje).

## 4. Pristupi implementaciji

| Pristup | Opis | Prednosti | Mane |
|---|---|---|---|
| **(a) Direktni HTTP iz Rust-a** | `ureq` (već je zavisnost, koristi se za RFZO) poziva isti endpoint kao web forma, parsira JSON/HTML | Potpuno automatski, rezultat u UI-ju | Radi samo gde nema CAPTCHA; krhko (promena sajta lomi); pravno siva zona bez saglasnosti |
| **(b) Webview / pregledač uz popunjenu tablicu** | Tauri `WebviewWindow` sa spoljnim URL-om ili sistemski pregledač; tablica u URL-u (ako je podržano) ili u clipboard-u | Radi svuda, korisnik rešava Turnstile/CAPTCHA, nema pravnih dilema, minimalno održavanje | Korisnik mora da klikne/pročita sam; nema statusa „čisto/ima kazni" u aplikaciji |
| **(c) Zvanični API / B2B ugovor** | Dogovor sa JKP-om ili Ministarstvom | Stabilno i legalno | Trenutno ne postoji; spor proces; za open-source desktop malo verovatno |
| **(d) Hibrid** | (a) za izvore sa otvorenim JSON-om (uz saglasnost), (b) za sve ostale, sve opisano u konfiguraciji | Najbolji odnos koristi i rizika | Malo više koda (dva tipa provajdera) |

Napomena o (b) u embedded webview-u: Tauri v2 može otvoriti prozor sa spoljnim URL-om i
`initialization_script`-om koji bi popunio polje za tablicu. Turnstile u WebView2/WKWebView obično
prolazi, a u WebKitGTK ponekad pravi probleme [P]. Čitanje rezultata iz tuđe stranice skriptom bi bilo
„scraping uz čoveka" — ne preporučuje se za MVP. Sistemski pregledač je najjednostavniji i
najpouzdaniji (korisnik ima kolačiće, Turnstile radi normalno).

### Preporuka po izvoru

| Izvor | Preporuka | Faza |
|---|---|---|
| Parking servis Beograd (eDPK) | **(b)** sistemski pregledač, URL sa tablicom ako parametar radi + tablica u clipboard; paralelno (c) zahtev za API | MVP |
| ParkingNS (Novi Sad + ~29 gradova) | **(a)** `checkPPK` JSON, **opt-in**, posle provere uslova/saglasnosti; inače (b) | Faza 2 (MVP kao (b)) |
| Niš, Kragujevac, Zrenjanin, Zlatibor, Prokuplje | **(b)**; (a) pojedinačno tek nakon analize forme i saglasnosti | MVP (b) |
| Čačak | (b) samo link — provera je po broju karte, ne tablici | MVP |
| RNK (Min. pravde) | **(b)** link + opcionalno kopiranje JMBG/ime na izričit klik; nikad automatski | MVP |
| eUprava „Pregled saobraćajnih prekršaja" | **(b)** link; napomena da se može prijaviti sertifikatom sa lične karte | MVP |
| Putarina (ENP / Toll4All) | **(b)** link, prikaz samo za teretna/autobuska vozila ili uvek u „Ostalo" | MVP |
| Agregatori (eparkiranje.com i sl.) | **Ne integrisati** (treća strana, nema API, podaci bi išli posredniku); eventualno pomenuti u pomoći | — |

**Ukupna preporuka: hibrid (d), sa MVP-om koji je 100 % pristup (b)** — panel „Provera kazni" sa
listom zvaničnih portala, popunjenom tablicom i dugmetom „Otvori na portalu". Automatske provere (a)
dodavati pojedinačno, kao opt-in, samo za izvore bez CAPTCHA i po mogućstvu uz pisanu saglasnost operatera.

## 5. Plan po fazama

**Faza 1 — MVP (link-out, bez slanja podataka iz backenda)**
1. `providers/fines.json` ugrađen u binarku (`include_str!`), tip provajdera `link`.
2. Normalizacija tablice (uppercase, bez razmaka i crtica; čuvati Č/Ć/Š/Ž/Đ — forum Garaža pominje
   probleme sa „Š" kod validatora jednog agregatora).
3. Svelte panel „Provera kazni" na prikazu saobraćajne dozvole; dugme „Otvori na portalu"
   (`tauri-plugin-opener`) i „Kopiraj tablicu".
4. Grad iz prefiksa tablice (BG, NS, NI, KG, ČA…) predlaže redosled; ostali gradovi su ispod.
5. Podešavanja: uključi/isključi panel, izbor gradova/izvora.

**Faza 2 — automatska provera tamo gde je moguće (opt-in)**
1. Tip provajdera `json_get` (deklarativno: URL šablon, metoda, mapiranje odgovora → broj stavki/iznos).
2. Prvi kandidat: ParkingNS `checkPPK` — nakon ručne verifikacije endpointa, uslova i (po mogućstvu) saglasnosti.
3. Dijalog saglasnosti pri prvom uključivanju; provera samo pri ubacivanju kartice ili na klik.
4. Keš u memoriji, statusi i greške (v. §7).

**Faza 3 — ažurne definicije i proširenja**
1. Preuzimanje `fines.json` iz repozitorijuma (potpisano), v. §8.
2. Dodatni gradovi kada se njihove forme analiziraju.
3. Zahtevi za zvanični API (Parking servis Beograd, ParkingNS) — ako uspe, novi tip `api`.
4. Opcionalno: embedded webview sa automatskim popunjavanjem polja (bez automatskog slanja).

## 6. UX predlog

Panel **„Provera kazni"** ispod podataka o vozilu:

```
┌ Provera kazni — BG123AB ─────────────────────────── [Proveri ponovo] ┐
│ Parking                                                                  │
│  ● Beograd (Parking servis, eDPK)   Potrebna ručna provera  [Otvori ↗]  │
│  ✓ Novi Sad i ParkingNS gradovi     Nema izdatih karata (pre 2 min)      │
│  ! Niš (eDPK)                       Nedostupno — pokušaj kasnije [Otvori ↗]
│  ▸ Ostali gradovi (Kragujevac, Zrenjanin, Zlatibor, Čačak…)              │
│ Saobraćajni prekršaji i sudske kazne (vezano za osobu, ne vozilo)        │
│  ● Registar neplaćenih kazni (vlasnik: P. Petrović)  [Otvori ↗] [Kopiraj JMBG]
│  ● eUprava – Pregled saobraćajnih prekršaja (prijava ConsentID/sertifikat) [Otvori ↗]
│ Putarina                                                                 │
│  ● ENP / Toll4All nalog                                       [Otvori ↗]  │
│ ⓘ Chitach ne čuva ove podatke i ne šalje ih nikome osim zvaničnim portalima. │
└──────────────────────────────────────────────────────────────────────────┘
```

- Statusi: ✓ čisto, ⚠ pronađeno N stavki / iznos (sa linkom za plaćanje na portalu), ● ručna
  provera, ! greška/nedostupno, — isključeno.
- Za JMBG: dugme za kopiranje skriva vrednost i prikazuje upozorenje pre kopiranja.
- Nikada ne prikazivati „nema kazni" za izvor koji nije stvarno proveren.
- Savet u panelu: „Za stalna obaveštenja koristite zvaničnu aplikaciju Parking servisa".

**Podešavanja** (`settings.json`, nova sekcija `fines`):

```json
"fines": {
  "enabled": true,
  "autoCheckOnInsert": false,
  "consentGiven": false,
  "consentVersion": 1,
  "enabledProviders": ["bg-parking", "parkingns", "nis-parking", "rnk", "euprava-prekrsaji", "enp"],
  "remoteProviders": true
}
```

Saglasnost (prikazuje se pre prve automatske provere): „Chitach će poslati registarsku oznaku
vozila zvaničnim portalima izabranih parking servisa. Podaci se ne čuvaju i ne šalju trećim licima.
Ako aplikaciju koristite u poslovne svrhe, vi ste rukovalac podacima u smislu ZZPL-a."

## 7. Arhitektura, keš i greške

- Novi modul `src-tauri/src/fines.rs` (po uzoru na `rfzo.rs` / `update.rs`, `ureq` sa timeout-om 10 s),
  komande `fines_providers(plate)` i `fines_check(provider_id, plate)`; rezultat kao događaj
  `fines-result` da UI ne blokira; svaki provajder u svojoj niti/tasku, paralelno.
- Model rezultata:
  ```rust
  enum FineStatus { Clean, Found { count: u32, total_rsd: Option<f64> },
                    ManualOnly, Unavailable { reason: String }, ParseError, Disabled }
  ```
- **Keš**: samo u memoriji, ključ `(provider_id, plate)`, TTL 30 min; ne pisati na disk (lični podaci);
  „Proveri ponovo" zaobilazi keš, uz minimalni razmak od 60 s po izvoru.
- **Greške**: timeout/HTTP 5xx → `Unavailable`; HTTP 403/429 ili HTML umesto JSON-a (verovatno
  CAPTCHA/zaštita) → automatski pad na `ManualOnly` + link i privremeno isključenje automatike za taj
  izvor; neočekivan JSON → `ParseError` + poruka „Izvor je možda promenio format; ažurirajte definicije".
- Ne logovati tablice i JMBG (ili ih maskirati: `BG1***B`).
- Identifikacija: iskren `User-Agent` (npr. `Chitach/<verzija> (+https://github.com/vahidzekic/Chitach)`).

## 8. Ažurne definicije provajdera (JSON konfiguracija)

Fajl `providers/fines.json` u repozitorijumu; ugrađena kopija + preuzimanje novije verzije sa
`https://raw.githubusercontent.com/vahidzekic/Chitach/main/providers/fines.json` (ili kao asset
GitHub Release-a), keširano na disku, provera najviše jednom dnevno.

```json
{
  "schema": 1,
  "updated": "2026-10-15",
  "providers": [
    {
      "id": "bg-parking",
      "category": "parking",
      "name": { "sr-Latn": "Beograd – Parking servis (eDPK)", "en": "Belgrade – Parking servis (eDPK)" },
      "cities": ["BG"],
      "kind": "link",
      "url": "https://www.parking-servis.co.rs/lat/edpk",
      "plateParam": null,
      "note": { "sr-Latn": "Zaštićeno Turnstile proverom – rešava korisnik. Prikazuje poslednjih 60 dana." }
    },
    {
      "id": "parkingns",
      "category": "parking",
      "name": { "sr-Latn": "Novi Sad i gradovi na ParkingNS portalu (ePPK)" },
      "cities": ["NS"],
      "kind": "json_get",
      "url": "https://portal.parkingns.rs/portal/auth/checkPPK?platePr={plate}",
      "openUrl": "https://portal.parkingns.rs/",
      "response": { "type": "array", "emptyMeans": "clean", "amountField": null },
      "enabledByDefault": false
    },
    {
      "id": "rnk",
      "category": "court",
      "subject": "owner",
      "kind": "link",
      "url": "https://rnk.sipres.sud.rs/",
      "copyFields": ["owners_personal_no"],
      "sensitive": true
    }
  ]
}
```

Bezbednost konfiguracije:
- Preuzeti fajl **potpisati** (minisign/ed25519, isti mehanizam kao Tauri updater) i proveriti potpis
  javnim ključem ugrađenim u binarku; inače koristiti ugrađenu verziju.
- **Allowlist domena** kompajlirana u binarku (npr. `*.parking-servis.co.rs`, `portal.parkingns.rs`,
  `nisparking.rs`, `jkpsumadija.rs`, `rnk.sipres.sud.rs`, `euprava.gov.rs`, …) — kompromitovan config
  ne sme moći da preusmeri tablice ili JMBG na tuđi server. Samo `https`.
- Konfiguracija je **deklarativna** (`link`, `json_get`, kasnije `html_form`), bez izvršnog koda.
- `schema` verzija: aplikacija ignoriše provajdere nepoznatog `kind`-a.
- CI zadatak (nedeljno) koji proverava da li `link` URL-ovi vraćaju 200 i da li `json_get` endpointi
  odgovaraju očekivanom obliku (sa test tablicom), i otvara issue kada se nešto promeni.

## 9. Otvorena pitanja (za ručnu proveru pre implementacije)

1. Da li `parking-servis.co.rs/edpk?plate=…` (ili `?fine=…`) i danas popunjava polje? Tačan selektor polja.
2. Da li `portal.parkingns.rs/portal/auth/checkPPK` i dalje radi bez tokena; oblik JSON-a (polja iznosa,
   datuma, grada); da li obuhvata sve gradove ParkingNS sistema; uslovi korišćenja portala.
3. Mehanizam provere na `nisparking.rs`, `jkpsumadija.rs`, `zrenjanin.park4all.rs`, `edpk.kjpzlatibor.rs`
   (forma/XHR/CAPTCHA).
4. Da li `rnk.sipres.sud.rs` ima CAPTCHA i da li URL prihvata parametre.
5. `robots.txt` i uslovi korišćenja svih navedenih sajtova.
6. Kontakt sa JKP „Parking servis" Beograd i JKP „Parking servis" Novi Sad u vezi sa zvaničnim API-jem.

## 10. Izvori

Zvanični (viđeni kroz pretragu, ne učitani uživo):
- https://www.parking-servis.co.rs/eng/edpk , https://www.parking-servis.co.rs/lat/edpk/?print=pdf
- https://www.parking-servis.co.rs/storage/8770/Korisnicko-uputstvo-eSalter-v1.0---06022026.pdf
- https://portal.parkingns.rs/ , https://parkingns.rs/
- https://www.nisparking.rs/sr/ , https://nis.eparking.rs/ , https://eparking.rs/
- https://www.jkpsumadija.rs/sr/info/sektor-parkinga/provera-opstih-dnevnih-parking-karti.html
- http://status.parkingcacak.co.rs/ , https://parkingcacak.co.rs/
- https://euprava.gov.rs/saobraćajni-prekršaji
- https://mpravde.gov.rs/registar-neplacenih-novcanih-kazni-i-drugih-novcanih-iznosa , https://rnk.sipres.sud.rs/
- https://www.paragraf.rs/propisi/pravilnik-o-pristupu-podacima-iz-jedinstvenog-registra-neplacenih-novcanih-kazni.html
- https://www.putevi-srbije.rs/index.php/sr/ , https://enp.amss.org.rs/
- https://www.poverenik.rs/sr-yu/saopstenja/2649-snimanje-vozila-u-saobra%C4%87aju-i-obrada-podataka-o-li%C4%8Dnosti.html

Javni kod (GitHub):
- github.com/s-kovacevic/ns-parking-ticket-checker — `checkPPK?platePr=`
- github.com/delicb/parking-service-tickets-scraper — `parking-servis.co.rs/edpk/?fine=`
- github.com/dev-cyprium/kazna-servis-reimagined-be — `ParkingservisService.php`

Treće strane:
- https://eparkiranje.com/ , https://eparkiranje.com/gradovi/beograd/ , https://eparkiranje.com/poslovno-resenje/ , https://eparkiranje.com/about/
- https://forum.xn--garaa-xib.rs/viewtopic.php?p=1153040 (tema „Centralni portal za proveru parking karata")
- https://kaznaservis.rs/ , https://www.jaoparking.com/
- https://www.b92.net/lokal/beograd/drustvo-kako-proveriti-kaznu-za-oko-sokolovo-2426767
- https://dozvola.rs/alati/provera-kaznenih-poena , https://akta.rs/alati/provera-kazni , https://600.rs/registar-neplacenih-kazni/
- https://forbes.n1info.rs/vesti/od-1-jula-kazne-za-teretna-vozila-koja-ne-koriste-tag-uredjaje-za-placanje-putarine/
- https://www.021.rs/story/Novi-Sad/Vesti/362525/Novosadjani-da-znate-Od-15-januara-elektronska-naplata-parkiranja-evo-sta-to-znaci.html

## 11. Šta je implementirano (faze 1 i 2)

| Stavka | Gde | Napomena |
|---|---|---|
| Deklarativne definicije provajdera, ugrađene `include_str!` | `src-tauri/providers/fines.json` | 12 izvora: Beograd, ParkingNS (Novi Sad, automatski), Novi Pazar na ParkingNS portalu (`ppkNP`, link) [U], Niš, Kragujevac, Zrenjanin, Zlatibor (`UE`), Prokuplje (`PK`), Čačak (samo link, po broju karte), RNK (samo link, JMBG se nikad ne šalje), eUprava (link), ENP/Toll4All (link). URL-ovi samo iz ovog dokumenta; kod nepoznatog parametra za tablicu — obična adresa + tablica u clipboard. |
| Normalizacija tablice i oznaka grada | `src-tauri/src/fines.rs` (`normalize_plate`, `city_code`) | velika slova, bez razmaka/crtica/tačaka, ćirilica → latinica (Ч→Č, Ђ→Đ, Љ→LJ, Џ→DŽ…); Č/Ć/Š/Ž/Đ se čuvaju; u URL-u se procentno kodira. Provajderi grada iz tablice su prvi. |
| Komande `fines_providers`, `fines_check` | `src-tauri/src/commands.rs` | v. `docs/04-api-ugovor.md`. |
| Panel „Provera kazni" | `ui/src/lib/pages/document/FinesPanel.svelte` | grupe parking / osoba / putarina, „Grad tablice", „Ostali gradovi", „Otvori na portalu", „Kopiraj tablicu", statusi i tabela kazni. |
| Automatska provera (opt-in) | `fines.rs` (`fetch`, `parse_json_array`, `FinesCache`) | samo `json_get` (ParkingNS `checkPPK`), `ureq` u `spawn_blocking`, timeout 12 s, `User-Agent: Chitach/<verzija> (+repo)`, jedan GET po proveri, bez ponavljanja; 403/429/HTML → „Proveri ručno", mrežna greška/5xx → „Nedostupno", neočekivan JSON → „Neočekivan odgovor". Keš u memoriji 30 min po (provajder, tablica); „Proveri ponovo" zaobilazi keš tek posle 60 s; tablica se u logu maskira. |
| Saglasnost i podešavanja | `settings.rs` (`fines: { autoCheck, consentGiven, enabledProviders? }`), `SettingsPage.svelte` | dijalog pre prve provere sa imenom domena; backend odbija proveru bez saglasnosti; opoziv u podešavanjima. |
| Mock | `CHITACH_MOCK=1`, `ui/src/lib/api/mockFines.ts` | bez mreže; isti `fines.json`. |

Format definicije (razlikuje se od skice u §8 — imena i napomene se prevode ključevima
`fines.provider.<id>.name/.note` u `ui/src/lib/i18n/*.json`, a `name` je rezervni naziv):

```json
{ "id": "np-parking", "category": "parking", "name": "Novi Pazar – Parking servis (ParkingNS portal)",
  "portal": "parkingns", "route": "ppkNP", "cities": ["NP"],
  "kind": "link", "url": "https://portal.parkingns.rs/{route}" }
```

Višegradski portali (ParkingNS) modeluju se kao po jedan provajder za grad sa zajedničkim
`portal` i gradskim kodom `route`; `{route}` (i `{city}` = prva oznaka grada iz `cities`) se
zamenjuju i u `url` i u `check.url`. Kad se potvrdi tačan zahtev za `ppkNP`, Novi Pazar postaje
automatski samo izmenom JSON-a, npr. (oblik parametra je **pretpostavka [P]**):

```json
{ "id": "np-parking", "portal": "parkingns", "route": "ppkNP", "cities": ["NP"], "kind": "json_get",
  "url": "https://portal.parkingns.rs/{route}",
  "check": { "url": "https://portal.parkingns.rs/portal/auth/checkPPK?platePr={plate}&<param>={route}", "parser": "jsonArray" } }
```

Prelazak nekog izvora na automatsku proveru je izmena JSON-a — `kind: "json_get"` i blok
`check` (`url` sa `{plate}`, samo `https`, i `parser`); `url` ostaje stranica za korisnika:

```json
{ "id": "parkingns", "kind": "json_get", "url": "https://portal.parkingns.rs/",
  "check": { "url": "https://portal.parkingns.rs/portal/auth/checkPPK?platePr={plate}", "parser": "jsonArray" } }
```

Parser `jsonArray` (prazan niz = čisto, stavke = kazne; prihvata i objekat sa jednim nizom,
polja datuma/iznosa/lokacije/opisa se prepoznaju heuristički, a sva skalarna polja se čuvaju
sirova) pokriva svaki izvor koji vraća JSON niz. Za drugačiji odgovor (npr. HTML forma za
HTML forma) dodaje se nova vrednost `parser` i mala funkcija u `fines.rs`. Nepoznat `kind` ili
`parser`, `json_get` bez `https`/`{plate}`, `{route}` bez `route` i neispravni URL-ovi se
ignorišu pri učitavanju.

Odstupanja od predloga: oznaka grada za Zlatibor/Čajetinu je `UE` (Užice); Novi Pazar je
poseban provajder (`NP`, link na `ppkNP`) dok se ne potvrdi API zahtev za taj grad; podešavanja nemaju
`enabled`/`consentVersion`/`remoteProviders` (panel je uvek prikazan, nema preuzimanja definicija).
