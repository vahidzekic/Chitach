# Plan implementacije

Paralelni tokovi (izvršavaju se istovremeno preko odvojenih agenata):

| Tok | Zadaci | Zavisi od |
|---|---|---|
| **T1 – Backend jezgro** | Go modul, adapter ka bas-celik `card`/`document`, DTO model, izvoz PDF/JSON/XLSX, promena PIN-a, RFZO, testovi | – |
| **T2 – Čitač/poller** | Praćenje čitača i kartica, Wails događaji `reader:list`, `card:inserted`, `card:removed`, `card:error` | ugovor API-ja (dole) |
| **T3 – Frontend** | Svelte + TS UI: layout, početni ekran, 3 prikaza dokumenata, podešavanja, i18n (sr-Latn/sr-Cyrl/en), tamna/svetla tema | ugovor API-ja |
| **T4 – Pakovanje i CI/CD** | Ikone, GitHub Actions: test + build za 5 ciljeva, DMG, NSIS, deb/rpm/AppImage, automatski Release | – |
| **T5 – CLI + dokumentacija** | `chitach-cli`, README (sr/en), uputstvo za korisnike, CONTRIBUTING | T1 |

## API ugovor (Go ↔ frontend)

```ts
// metode (Wails bindings)
ListReaders(): Promise<Reader[]>
SelectReader(name: string): Promise<void>
GetCurrentDocument(): Promise<DocumentView | null>
ExportPDF(): Promise<string>        // vraća putanju, otvara Save dijalog
ExportJSON(): Promise<string>
ExportXLSX(): Promise<string>
UpdateMedicalValidity(): Promise<DocumentView>   // RFZO
ChangePIN(oldPin: string, newPin: string): Promise<void>
GetSettings(): Promise<Settings>
SaveSettings(s: Settings): Promise<void>
GetVersion(): Promise<string>

// događaji
"readers:changed"  -> Reader[]
"card:state"       -> { state: "no-reader"|"waiting"|"reading"|"ready"|"error", message?: string }
"document:loaded"  -> DocumentView

type DocumentView = {
  kind: "id" | "medical" | "vehicle" | "unknown",
  title: string,
  photo?: string,                 // data:image/jpeg;base64,...
  groups: { title: string, fields: { label: string, value: string, key: string }[] }[]
}
```

## Faze

1. **Faza 0** – istraživanje i dokumentacija ✅ (ovaj dokument)
2. **Faza 1** – izbor jezika (odluka korisnika)
3. **Faza 2** – skelet projekta + API ugovor (sekvencijalno, ~1 korak)
4. **Faza 3** – T1–T5 paralelno
5. **Faza 4** – integracija, build na Linuksu lokalno, testovi, pregled koda
6. **Faza 5** – push na `git`, tag `v0.1.0` → GitHub Actions pravi Release sa instalacijama

## Rizici

* **Nema fizičkog čitača u CI** – logika parsiranja se testira golden podacima iz Baš Čelika; UI se testira mock režimom (`CHITACH_MOCK=1` prikazuje primer dokumenta).
* **macOS potpisivanje** – bez Apple naloga build je ad-hoc potpisan.
* **Windows SmartScreen** – nepotpisan `.exe` daje upozorenje; rešava se kupovinom code-signing sertifikata.
* **Smartbox i PKCS#11 potpisivanje** – van opsega v0.1, planirano za v0.2.
