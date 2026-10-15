// Sample documents for the mock API (development and screenshots only).
// All data is fictional.
import type { CertInfo, IdDocument, MedicalDocument, TokenCert, VehicleDocument } from "./types";

/** Draws a neutral illustrated portrait and returns it as base64 JPEG (like the backend). */
export function makeMockPortrait(): string {
  try {
    const c = document.createElement("canvas");
    c.width = 300;
    c.height = 380;
    const g = c.getContext("2d");
    if (!g) return "";
    const bg = g.createLinearGradient(0, 0, 0, 380);
    bg.addColorStop(0, "#dfe6ef");
    bg.addColorStop(1, "#b9c6d6");
    g.fillStyle = bg;
    g.fillRect(0, 0, 300, 380);
    // shoulders
    g.fillStyle = "#3d4b5f";
    g.beginPath();
    g.ellipse(150, 400, 135, 120, 0, Math.PI, 0);
    g.fill();
    // collar
    g.fillStyle = "#f3f4f6";
    g.beginPath();
    g.moveTo(118, 290);
    g.lineTo(150, 335);
    g.lineTo(182, 290);
    g.closePath();
    g.fill();
    // neck
    g.fillStyle = "#e0b394";
    g.fillRect(128, 220, 44, 75);
    // head
    g.beginPath();
    g.ellipse(150, 170, 62, 78, 0, 0, Math.PI * 2);
    g.fill();
    // hair
    g.fillStyle = "#4a3426";
    g.beginPath();
    g.ellipse(150, 118, 66, 40, 0, Math.PI, 0);
    g.fill();
    g.fillRect(84, 112, 14, 42);
    g.fillRect(202, 112, 14, 42);
    // eyes
    g.fillStyle = "#2d2a28";
    g.beginPath();
    g.ellipse(127, 170, 6, 4, 0, 0, Math.PI * 2);
    g.ellipse(173, 170, 6, 4, 0, 0, Math.PI * 2);
    g.fill();
    // brows
    g.strokeStyle = "#4a3426";
    g.lineWidth = 4;
    g.beginPath();
    g.moveTo(114, 155);
    g.lineTo(138, 152);
    g.moveTo(162, 152);
    g.lineTo(186, 155);
    g.stroke();
    // mouth
    g.strokeStyle = "#a86a5a";
    g.lineWidth = 3;
    g.beginPath();
    g.arc(150, 205, 16, 0.2 * Math.PI, 0.8 * Math.PI);
    g.stroke();
    const url = c.toDataURL("image/jpeg", 0.9);
    return url.substring(url.indexOf(",") + 1);
  } catch {
    return "";
  }
}

export function sampleId(): IdDocument {
  return {
    Portrait: makeMockPortrait(),
    DocRegNo: "012345678",
    DocumentType: "ID",
    IssuingDate: "10.04.2024.",
    ExpiryDate: "10.04.2034.",
    IssuingAuthority: "PU ZA GRAD BEOGRAD",
    DocumentSerialNumber: "0001234567",
    ChipSerialNumber: "3B4C5D6E7F801234",
    DocumentName: "",
    PersonalNumber: "0101990710006",
    Surname: "PETROVIĆ",
    GivenName: "MILICA",
    ParentGivenName: "JOVAN",
    Sex: "Ž",
    PlaceOfBirth: "BEOGRAD",
    CommunityOfBirth: "ZVEZDARA",
    StateOfBirth: "REPUBLIKA SRBIJA",
    StateOfBirthCode: "SRB",
    DateOfBirth: "01.01.1990.",
    StatusOfForeigner: "",
    NationalityFull: "",
    PurposeOfStay: "",
    ENote: "",
    State: "SRB",
    Community: "VRAČAR",
    Place: "BEOGRAD",
    Street: "KRUNSKA",
    HouseNumber: "12",
    HouseLetter: "A",
    Entrance: "",
    Floor: "3",
    ApartmentNumber: "9",
    AddressDate: "15.06.2021.",
    AddressLabel: "",
  };
}

export function sampleMedical(): MedicalDocument {
  return {
    InsurerName: "Републички фонд за здравствено осигурање",
    InsurerID: "61000",
    CardId: "00012345678",
    DateOfIssue: "01.02.2022.",
    DateOfExpiry: "01.02.2032.",
    ChipSerialNumber: "8099123456789012",
    PrintLanguage: "",
    PersonalNumber: "0101990710006",
    FamilyNameLatin: "Petrović",
    GivenNameLatin: "Milica",
    ParentNameLatin: "Jovan",
    FamilyName: "Петровић",
    GivenName: "Милица",
    ParentName: "Јован",
    Gender: "Женски",
    InsurantNumber: "12345678901",
    DateOfBirth: "01.01.1990.",
    Apartment: "9",
    Number: "12a",
    Entrance: "",
    Street: "Крунска",
    StreetCode: "12021",
    Place: "Београд",
    PostNumber: "11000",
    Municipality: "Врачар",
    Country: "Србија",
    ValidUntil: "31.12.2026.",
    PermanentlyValid: false,
    CarrierGivenNameLatin: "Milica",
    CarrierFamilyNameLatin: "Petrović",
    CarrierGivenName: "Милица",
    CarrierFamilyName: "Петровић",
    CarrierIdNumber: "0101990710006",
    CarrierInsurantNumber: "12345678901",
    CarrierFamilyMember: false,
    CarrierRelationship: "",
    InsuranceBasisRZZO: "10101",
    InsuranceStartDate: "15.06.2021.",
    InsuranceDescription: "Запослени у привредном друштву",
    TaxpayerName: "Пример д.о.о. Београд",
    TaxpayerResidence: "Београд",
    TaxpayerNumber: "20123456",
    TaxpayerIdNumber: "100123456",
    TaxpayerActivityCode: "6201",
  };
}

export function sampleVehicle(): VehicleDocument {
  return {
    AuthorityIssuing: "MUP R. SRBIJE",
    ColourOfVehicle: "SIVA METALIK",
    CommercialDescription: "OCTAVIA",
    CompetentAuthority: "PU ZA GRAD BEOGRAD",
    DateOfFirstRegistration: "15.01.2020",
    EngineCapacity: "1968",
    EngineIdNumber: "DFG123456",
    ExpiryDate: "31.12.2026",
    HomologationMark: "E9",
    IssuingDate: "10.01.2025",
    MaximumNetPower: "110",
    MaximumPermissibleLadenMass: "1950",
    NumberOfAxles: "2",
    NumberOfSeats: "5",
    NumberOfStandingPlaces: "",
    OwnerAddress: "BEOGRAD, VRAČAR, KRUNSKA 12A",
    OwnerName: "MILICA",
    OwnersPersonalNo: "0101990710006",
    OwnersSurnameOrBusinessName: "PETROVIĆ",
    PowerWeightRatio: "0.08",
    RegistrationNumberOfVehicle: "BG1234AB",
    SerialNumber: "001234567",
    StateIssuing: "REPUBLIKA SRBIJA",
    TypeApprovalNumber: "e9*2007/46*0123*05",
    TypeOfFuel: "DIZEL",
    UnambiguousNumber: "SRB1234567",
    UsersAddress: "",
    UsersName: "",
    UsersPersonalNo: "",
    UsersSurnameOrBusinessName: "",
    VehicleCategory: "M1",
    VehicleIdNumber: "TMBJJ7NE0L0123456",
    VehicleLoad: "475",
    VehicleMake: "ŠKODA",
    VehicleMass: "1475",
    VehicleType: "PUTNIČKO VOZILO",
    YearOfProduction: "2019",
  };
}

export const sampleCerts: CertInfo[] = [
  {
    index: 0,
    label: "Sertifikat za autentifikaciju",
    subject: "CN=MILICA PETROVIĆ 0101990710006, SERIALNUMBER=PNOSR-0101990710006, C=RS",
    issuer: "CN=MUPCA Gradjani, O=MUP Republike Srbije, C=RS",
    notAfter: "2029-04-10 12:00:00",
  },
  {
    index: 1,
    label: "Kvalifikovani sertifikat za potpis",
    subject: "CN=MILICA PETROVIĆ 0101990710006, SERIALNUMBER=PNOSR-0101990710006, C=RS",
    issuer: "CN=MUPCA Gradjani, O=MUP Republike Srbije, C=RS",
    notAfter: "2029-04-10 12:00:00",
  },
];

export const sampleTokenCerts: TokenCert[] = [
  {
    id: "a1b2c3d4e5f60718",
    label: "Milica Petrović (potpis)",
    subject: "CN=MILICA PETROVIĆ, C=RS",
    issuer: "CN=MUPCA Gradjani, O=MUP Republike Srbije, C=RS",
    notAfter: "2029-04-10 12:00:00",
  },
  {
    id: "0f1e2d3c4b5a6978",
    label: "Milica Petrović (autentifikacija)",
    subject: "CN=MILICA PETROVIĆ, C=RS",
    issuer: "CN=MUPCA Gradjani, O=MUP Republike Srbije, C=RS",
    notAfter: "2029-04-10 12:00:00",
  },
];
