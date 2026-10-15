#!/usr/bin/env bash
# Sets up macOS signing and notarization for the Release workflow (docs/06-razvoj.md).
#
# Run on the Mac that has the "Developer ID Application" certificate in its login keychain:
#   bash scripts/setup-macos-signing.sh
#
# What it does:
#   1. exports the newest "Developer ID Application" certificate with its private key to a
#      temporary .p12 (macOS asks to allow access to the key: click "Always Allow" / enter
#      the Mac password),
#   2. asks for the App Store Connect API key (.p8 file and Issuer ID) for notarization,
#   3. stores everything as GitHub Actions secrets with the GitHub CLI (`gh`).
# Nothing is written outside a temporary folder, which is deleted at the end.
set -euo pipefail

REPO="${CHITACH_REPO:-vahidzekic/Chitach}"
OPENSSL=/usr/bin/openssl # LibreSSL: its .p12 files import on every macOS version

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
die() { printf '\n\033[31mGreška: %s\033[0m\n' "$*" >&2; exit 1; }

[ "$(uname)" = Darwin ] || die "skripta radi samo na macOS-u."
[ -x "$OPENSSL" ] || OPENSSL=openssl

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# ---------- GitHub CLI ----------
say "1/4  GitHub CLI"
if ! command -v gh >/dev/null; then
  if command -v brew >/dev/null; then
    echo "Instaliram gh (GitHub CLI) preko Homebrew-a…"
    brew install gh
  else
    die "nema 'gh' ni 'brew'. Instaliraj GitHub CLI sa https://cli.github.com i pokreni skriptu ponovo."
  fi
fi
if ! gh auth status >/dev/null 2>&1; then
  echo "Prijava na GitHub: otvoriće se pregledač, potvrdi kod koji se ispiše."
  gh auth login --hostname github.com --git-protocol https --web
fi
gh repo view "$REPO" --json name >/dev/null || die "nemam pristup repozitorijumu $REPO."
echo "OK – $REPO"

# ---------- certificate ----------
say "2/4  Developer ID Application sertifikat"
security find-identity -v -p codesigning | grep -q "Developer ID Application" ||
  die "u Keychain-u nema 'Developer ID Application' sertifikata."

export_pass="$("$OPENSSL" rand -hex 16)"
echo "macOS će tražiti dozvolu za pristup ključevima: klikni 'Always Allow' (ili upiši lozinku Mac-a)."
security export -k login.keychain-db -t identities -f pkcs12 -P "$export_pass" -o "$work/all.p12" ||
  security export -k login.keychain -t identities -f pkcs12 -P "$export_pass" -o "$work/all.p12"
"$OPENSSL" pkcs12 -in "$work/all.p12" -passin "pass:$export_pass" -nodes -out "$work/all.pem" 2>/dev/null ||
  die "ne mogu da pročitam izvezene ključeve."

# Split the PEM into one file per bag (cert or key) and pick the newest Developer ID cert.
awk -v dir="$work" '
  /^Bag Attributes/ { n++ }
  n { print > (dir "/bag" n ".txt") }
' "$work/all.pem"

best="" best_end=0 best_id=""
for f in "$work"/bag*.txt; do
  grep -q "BEGIN CERTIFICATE" "$f" || continue
  subject="$("$OPENSSL" x509 -in "$f" -noout -subject 2>/dev/null)" || continue
  case "$subject" in *"Developer ID Application"*) ;; *) continue ;; esac
  end="$("$OPENSSL" x509 -in "$f" -noout -enddate | cut -d= -f2)"
  end_s="$(date -j -f "%b %e %T %Y %Z" "$end" +%s 2>/dev/null || date -d "$end" +%s)"
  id="$(grep -m1 'localKeyID' "$f" | sed 's/.*localKeyID: *//')"
  [ -n "$id" ] || continue
  if [ "$end_s" -gt "$best_end" ]; then best="$f" best_end="$end_s" best_id="$id"; fi
done
[ -n "$best" ] || die "nisam našao Developer ID Application sertifikat sa privatnim ključem."

key=""
for f in "$work"/bag*.txt; do
  grep -q "PRIVATE KEY" "$f" || continue
  grep -q "localKeyID: *$best_id" "$f" && key="$f" && break
done
[ -n "$key" ] || die "nema privatnog ključa za izabrani sertifikat."

subject="$("$OPENSSL" x509 -in "$best" -noout -subject -nameopt multiline)"
identity="$(printf '%s\n' "$subject" | sed -n 's/^ *commonName *= *//p' | head -n1)"
team_id="$(printf '%s\n' "$subject" | sed -n 's/^ *organizationalUnitName *= *//p' | head -n1)"
[ -n "$identity" ] || die "ne mogu da pročitam ime sertifikata."

p12_pass="$("$OPENSSL" rand -hex 16)"
"$OPENSSL" pkcs12 -export -in "$best" -inkey "$key" -name "$identity" \
  -passout "pass:$p12_pass" -out "$work/cert.p12" 2>/dev/null || die "ne mogu da napravim .p12."
echo "Sertifikat: $identity (važi do $(date -r "$best_end" +%Y-%m-%d))"

# ---------- notarization key ----------
say "3/4  App Store Connect API ključ (za notarizaciju)"
cat <<'EOF'
Ako ga još nemaš: appstoreconnect.apple.com → Users and Access → Integrations →
App Store Connect API → Team Keys → "+" → ime "Chitach CI", uloga Developer → Generate,
pa "Download API Key" (preuzima se samo jednom!). Issuer ID piše iznad tabele ključeva.
EOF
p8=""
read -r -p "Prevuci AuthKey_XXXX.p8 fajl ovde i pritisni Enter (ili samo Enter da preskočiš): " p8
p8="${p8%\"}"; p8="${p8#\"}"; p8="${p8%\'}"; p8="${p8#\'}"; p8="${p8%"${p8##*[![:space:]]}"}"
p8="${p8//\\ / }"
if [ -n "$p8" ]; then
  [ -f "$p8" ] || die "fajl ne postoji: $p8"
  grep -q "BEGIN PRIVATE KEY" "$p8" || die "to nije .p8 ključ: $p8"
  key_id="$(basename "$p8" | sed -n 's/^AuthKey_\([A-Z0-9]*\)\.p8$/\1/p')"
  [ -n "$key_id" ] || read -r -p "Key ID (10 znakova): " key_id
  read -r -p "Issuer ID (oblik xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx): " issuer
  [[ "$issuer" =~ ^[0-9a-fA-F-]{36}$ ]] || die "Issuer ID nije ispravan."
else
  echo "Preskačem: aplikacija će biti potpisana, ali ne i notarizovana (skriptu možeš pokrenuti ponovo)."
fi

# ---------- secrets ----------
say "4/4  Upisujem tajne u GitHub ($REPO)"
put() { printf '%s' "$2" | gh secret set "$1" --repo "$REPO" >/dev/null && echo "  ✓ $1"; }
put APPLE_CERTIFICATE "$(base64 < "$work/cert.p12" | tr -d '\n')"
put APPLE_CERTIFICATE_PASSWORD "$p12_pass"
put APPLE_SIGNING_IDENTITY "$identity"
[ -n "$team_id" ] && put APPLE_TEAM_ID "$team_id"
if [ -n "$p8" ]; then
  put APPLE_API_KEY "$key_id"
  put APPLE_API_ISSUER "$issuer"
  put APPLE_API_KEY_P8 "$(cat "$p8")"
fi

say "Gotovo."
echo "Sledeći tag (npr. v0.3.0) gradi potpisanu$( [ -n "$p8" ] && echo " i notarizovanu") macOS verziju."
echo "Spisak tajni: https://github.com/$REPO/settings/secrets/actions"
