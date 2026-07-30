#!/bin/bash
# Postaví Ptáčka a zabalí ho do brandovaného DMG.
#
# Proč tenhle skript existuje a nestačí `npx tauri build`:
#
# 1. Bundle se musí jmenovat "Ptáček.app", ale spustitelný soubor uvnitř
#    "Ptacek". Diakritika ve jménu binárky rozbíjí podpis (codesign ji
#    zapečetí jako resource místo hlavního kódu), zatímco bez diakritiky
#    ve jménu složky Finder ukazuje "Ptacek" bez háčku — CFBundleDisplayName
#    ani lokalizovaný InfoPlist.strings s tím nic neudělají, Finder bere
#    jméno souboru. Tauri v1 umí nastavit jen productName pro obojí
#    najednou, takže se složka přejmenuje až po buildu. Podpisu to nevadí:
#    pečeť pokrývá Contents/, ne jméno složky.
# 2. DMG s pozadím a rozmístěním ikon staví appdmg, ne Tauri.
#
# POZOR na diakritiku v shellu: "Ptáček.app" existuje na disku v NFD, ale
# v tomhle souboru je zapsané v NFC — vzory typu rm -rf "Ptáček.app" pak
# tiše nic netrefí. Proto se všude maže přes find -iname "pt*ek.app".

set -euo pipefail
cd "$(dirname "$0")/.."

export PATH="$HOME/.cargo/bin:$PATH"   # cargo nebývá v PATH neinteraktivního shellu

BUNDLE_DIR="src-tauri/target/release/bundle/macos"
OUT="${1:-packaging/Ptáček.dmg}"

echo "▸ build"
npx tauri build

echo "▸ přejmenování bundlu na Ptáček.app"
find "$BUNDLE_DIR" -maxdepth 1 -iname "pt*ek.app" -not -name "Ptacek.app" -exec rm -rf {} +
cp -R "$BUNDLE_DIR/Ptacek.app" "$BUNDLE_DIR/Ptáček.app"

echo "▸ kontrola podpisu"
codesign --verify --strict "$BUNDLE_DIR/Ptáček.app"

echo "▸ DMG"
rm -f "$OUT"
npx --yes appdmg packaging/appdmg.json "$OUT"

echo "▸ hotovo: $OUT"
shasum -a 256 "$OUT"
