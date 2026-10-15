"""Generate the Chitach app icon (1024x1024 PNG) with PIL, supersampled 4x."""
import sys
from PIL import Image, ImageDraw, ImageFilter

S = 4
N = 1024 * S
out = sys.argv[1]


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def gradient(w, h, c1, c2):
    """Diagonal gradient top-left c1 -> bottom-right c2."""
    g = Image.new("RGBA", (w, h))
    small = Image.new("RGBA", (256, 256))
    px = small.load()
    for y in range(256):
        for x in range(256):
            t = (x + y) / 510
            px[x, y] = lerp(c1, c2, t)
    return small.resize((w, h), Image.BICUBIC)


def rrect_mask(size, box, r):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle(box, radius=r, fill=255)
    return m


canvas = Image.new("RGBA", (N, N), (0, 0, 0, 0))

# --- background tile (macOS-style margins: ~10% each side) ---
m = int(100 * S)
tile_box = (m, m, N - m, N - m)
tile_r = int(185 * S)
bg = gradient(N, N, (20, 184, 166, 255), (49, 46, 129, 255))  # teal -> indigo
tile_mask = rrect_mask((N, N), tile_box, tile_r)

# soft drop shadow under the tile
sh = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(sh).rounded_rectangle(
    (m, m + 18 * S, N - m, N - m + 18 * S), radius=tile_r, fill=(0, 0, 0, 90)
)
sh = sh.filter(ImageFilter.GaussianBlur(22 * S))
canvas = Image.alpha_composite(canvas, sh)
canvas.paste(bg, (0, 0), tile_mask)

# subtle top highlight
hl = Image.new("RGBA", (N, N), (0, 0, 0, 0))
hd = ImageDraw.Draw(hl)
hd.ellipse((-N * 0.2, -N * 0.75, N * 1.2, N * 0.45), fill=(255, 255, 255, 30))
hl = hl.filter(ImageFilter.GaussianBlur(60 * S))
hl_masked = Image.new("RGBA", (N, N), (0, 0, 0, 0))
hl_masked.paste(hl, (0, 0), tile_mask)
canvas = Image.alpha_composite(canvas, hl_masked)

# --- contactless waves (top-right) ---
waves = Image.new("RGBA", (N, N), (0, 0, 0, 0))
wd = ImageDraw.Draw(waves)
cx, cy = 690 * S, 330 * S
for i, rad in enumerate((70, 120, 170)):
    r = rad * S
    wd.arc(
        (cx - r, cy - r, cx + r, cy + r),
        start=-80,
        end=-10,
        fill=(255, 255, 255, 210 - i * 50),
        width=int(26 * S),
    )
waves_m = Image.new("RGBA", (N, N), (0, 0, 0, 0))
waves_m.paste(waves, (0, 0), tile_mask)

# --- the card (drawn on its own layer, then rotated) ---
L = Image.new("RGBA", (N, N), (0, 0, 0, 0))
cw, ch = 620 * S, 400 * S
x0 = (N - cw) // 2
y0 = (N - ch) // 2 + 60 * S
cr = 46 * S
card_box = (x0, y0, x0 + cw, y0 + ch)

# card shadow
cs = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(cs).rounded_rectangle(
    (x0, y0 + 26 * S, x0 + cw, y0 + ch + 26 * S), radius=cr, fill=(10, 10, 40, 120)
)
cs = cs.filter(ImageFilter.GaussianBlur(26 * S))

d = ImageDraw.Draw(L)
d.rounded_rectangle(card_box, radius=cr, fill=(255, 255, 255, 255))
# header band
band = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(band).rectangle(
    (x0, y0, x0 + cw, y0 + 78 * S), fill=(226, 232, 255, 255)
)
L.paste(band, (0, 0), Image.composite(band, Image.new("RGBA", (N, N)), rrect_mask((N, N), card_box, cr)).split()[3])
d = ImageDraw.Draw(L)
# small header lines
d.rounded_rectangle(
    (x0 + 44 * S, y0 + 30 * S, x0 + 300 * S, y0 + 48 * S), radius=9 * S, fill=(99, 102, 241, 255)
)

# portrait (left)
px0, py0 = x0 + 44 * S, y0 + 118 * S
pw, ph = 170 * S, 218 * S
d.rounded_rectangle((px0, py0, px0 + pw, py0 + ph), radius=22 * S, fill=(224, 231, 255, 255))
head_r = 42 * S
hx, hy = px0 + pw // 2, py0 + 82 * S
d.ellipse((hx - head_r, hy - head_r, hx + head_r, hy + head_r), fill=(79, 70, 229, 255))
body = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(body).ellipse(
    (hx - 78 * S, hy + 58 * S, hx + 78 * S, hy + 230 * S), fill=(79, 70, 229, 255)
)
pmask = rrect_mask((N, N), (px0, py0, px0 + pw, py0 + ph), 22 * S)
L.paste(body, (0, 0), Image.composite(body, Image.new("RGBA", (N, N)), pmask).split()[3])
d = ImageDraw.Draw(L)

# gold chip (right-top area)
gx0, gy0 = x0 + 250 * S, y0 + 118 * S
gw, gh = 132 * S, 104 * S
chip = gradient(gw, gh, (253, 224, 71, 255), (217, 119, 6, 255))
L.paste(chip, (gx0, gy0), rrect_mask((gw, gh), (0, 0, gw, gh), 20 * S))
line = (161, 98, 7, 255)
lw = int(7 * S)
# chip contact pattern
d.line((gx0, gy0 + gh // 3, gx0 + gw, gy0 + gh // 3), fill=line, width=lw)
d.line((gx0, gy0 + 2 * gh // 3, gx0 + gw, gy0 + 2 * gh // 3), fill=line, width=lw)
d.line((gx0 + gw // 2, gy0, gx0 + gw // 2, gy0 + gh // 3), fill=line, width=lw)
d.line((gx0 + gw // 2, gy0 + 2 * gh // 3, gx0 + gw // 2, gy0 + gh), fill=line, width=lw)
d.line((gx0 + gw // 2, gy0 + gh // 3, gx0 + gw // 2, gy0 + 2 * gh // 3), fill=line, width=lw)

# text lines
tx0 = x0 + 250 * S
for i, (wid, col) in enumerate(
    [(320, (49, 46, 129, 255)), (250, (148, 163, 184, 255)), (290, (148, 163, 184, 255))]
):
    ty = y0 + 252 * S + i * 42 * S
    d.rounded_rectangle((tx0, ty, tx0 + wid * S, ty + 20 * S), radius=10 * S, fill=col)

# rotate card + shadow slightly
angle = 6
L = L.rotate(angle, resample=Image.BICUBIC, center=(N // 2, N // 2 + 60 * S))
cs = cs.rotate(angle, resample=Image.BICUBIC, center=(N // 2, N // 2 + 60 * S))

canvas = Image.alpha_composite(canvas, waves_m)
canvas = Image.alpha_composite(canvas, cs)
canvas = Image.alpha_composite(canvas, L)

canvas = canvas.resize((1024, 1024), Image.LANCZOS)
canvas.save(out)
print("wrote", out)
