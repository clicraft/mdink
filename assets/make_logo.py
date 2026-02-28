#!/usr/bin/env python3
"""Generate assets/mdink-logo.png — gradient ASCII art logo."""

import sys, os

from PIL import Image, ImageDraw, ImageFont, ImageFilter

ART = [
    "███╗   ███╗██████╗ ██╗███╗   ██╗██╗  ██╗",
    "████╗ ████║██╔══██╗██║████╗  ██║██║ ██╔╝",
    "██╔████╔██║██║  ██║██║██╔██╗ ██║█████╔╝ ",
    "██║╚██╔╝██║██║  ██║██║██║╚██╗██║██╔═██╗ ",
    "██║ ╚═╝ ██║██████╔╝██║██║ ╚████║██║  ██╗",
    "╚═╝     ╚═╝╚═════╝ ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝",
]
TAGLINE = "terminal  markdown  renderer"

YELLOW     = (255, 235,   0)   # bright light yellow
VERMILLION = (218,  40,  15)   # bright saturated vermillion
BG         = ( 13,  13,  18)   # near-black, slightly blue

FONT_PATH = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
SCALE = 2   # render at 2× then downscale for clean antialiasing

FS_ART     = 28 * SCALE
FS_TAG     = 13 * SCALE
LINE_GAP   =  6 * SCALE
PAD_X      = 36 * SCALE
PAD_TOP    = 32 * SCALE
PAD_BOT    = 28 * SCALE
TAG_GAP    = 18 * SCALE
GLOW_R     =  9 * SCALE

def smoothstep(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)

def lerp(a, b, t):
    t = smoothstep(t)
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))

font_art = ImageFont.truetype(FONT_PATH, FS_ART)
font_tag = ImageFont.truetype(FONT_PATH, FS_TAG)

def text_size(text, font):
    bbox = font.getbbox(text)
    return bbox[2] - bbox[0], bbox[3] - bbox[1]

# Measure art block
art_widths  = [text_size(line, font_art)[0] for line in ART]
art_heights = [text_size(line, font_art)[1] for line in ART]
art_w = max(art_widths)
art_h = sum(art_heights) + LINE_GAP * (len(ART) - 1)

tag_w, tag_h = text_size(TAGLINE, font_tag)

canvas_w = art_w  + PAD_X * 2
canvas_h = art_h + TAG_GAP + tag_h + PAD_TOP + PAD_BOT

# ── Build horizontal gradient ─────────────────────────────────────
row = []
for x in range(canvas_w):
    t = (x - PAD_X) / max(art_w - 1, 1)
    row.extend(lerp(YELLOW, VERMILLION, t))
gradient = Image.frombytes("RGB", (canvas_w, 1), bytes(row))
gradient = gradient.resize((canvas_w, canvas_h), Image.NEAREST)

# ── Render text into a grayscale mask ────────────────────────────
mask = Image.new("L", (canvas_w, canvas_h), 0)
d    = ImageDraw.Draw(mask)

y = PAD_TOP
for i, line in enumerate(ART):
    d.text((PAD_X, y), line, font=font_art, fill=255)
    y += art_heights[i] + LINE_GAP

# Tagline — centred under the art
tx = PAD_X + (art_w - tag_w) // 2
d.text((tx, y + TAG_GAP), TAGLINE, font=font_tag, fill=160)

# ── Glow layer (blurred mask at reduced brightness) ───────────────
glow_mask = mask.filter(ImageFilter.GaussianBlur(radius=GLOW_R))

result = Image.new("RGB", (canvas_w, canvas_h), BG)
result.paste(gradient, mask=glow_mask)   # soft glow
result.paste(gradient, mask=mask)        # crisp text on top

# ── Downscale 2× → final size ────────────────────────────────────
final = result.resize(
    (canvas_w // SCALE, canvas_h // SCALE),
    Image.LANCZOS
)

out = "assets/mdink-logo.png"
final.save(out, optimize=True)
w, h = final.size
print(f"✓  {w}×{h}px  →  {out}")
