#!/usr/bin/env python3
"""Generates the store QR codes for the website (site/img/qr-*.svg).

Run again only if a store URL changes:  pip install segno && python3 scripts/site-store-qr.py
The URLs must match the ones in site/index.html (STORES).
"""
import pathlib
import segno

STORES = {
    "ios": "https://apps.apple.com/app/id6837721736",
    "android": "https://play.google.com/store/apps/details?id=rs.chitach.app",
}

out = pathlib.Path(__file__).resolve().parent.parent / "site" / "img"
out.mkdir(parents=True, exist_ok=True)
for name, url in STORES.items():
    qr = segno.make(url, error="m")
    path = out / f"qr-{name}.svg"
    qr.save(path, kind="svg", scale=4, border=2, dark="#000", light="#fff",
            xmldecl=False, svgns=True, title=url)
    print(path, qr.version)
