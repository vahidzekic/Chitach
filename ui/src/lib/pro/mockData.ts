// Sample certificates for the Pro mock API (development and screenshots only).
// All data is fictional.
import type { CertInfo, TokenCert } from "./types";

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
