

import os
from PIL import Image

SRC = rE:\QFNOTE\image.png"
ICONS = rE:\QFNOTE\src-tauri\icons"

def make_square(img, size, pad=0.0):
    """把图 contain 进 size x size 正方形画布（透明背景），可留白边 pad(0~0.2)。"""
    img = img.convert(RGBA")
    canvas = Image.new(RGBA", (size, size), (0, 0, 0, 0))
    iw, ih = img.size
    if iw == 0 or ih == 0:
        return canvas
    
    avail = int(size * (1.0 - pad))
    scale = min(avail / iw, avail / ih)
    nw, nh = max(1, round(iw * scale)), max(1, round(ih * scale))
    resized = img.resize((nw, nh), Image.LANCZOS)
    ox = (size - nw) // 2
    oy = (size - nh) // 2
    canvas.paste(resized, (ox, oy), resized)
    return canvas

def main():
    src = Image.open(SRC)
    print(source size:", src.size, mode:", src.mode)
    
    master = make_square(src, 512, pad=0.06)

    
    pn = {
        32x32.png": 32,
        128x128.png": 128,
        128x128@2x.png": 256,
        icon.png": 512,
        Square30x30Logo.png": 30,
        Square44x44Logo.png": 44,
        Square71x71Logo.png": 71,
        Square89x89Logo.png": 89,
        Square107x107Logo.png": 107,
        Square142x142Logo.png": 142,
        Square150x150Logo.png": 150,
        Square284x284Logo.png": 284,
        Square310x310Logo.png": 310,
        StoreLogo.png": 50,
    }
    for name, s in pn.items():
        out = make_square(src, s, pad=0.06)
        out.save(os.path.join(ICONS, name), PNG")
        print(wrote", name, s)

    
    ico_master = make_square(src, 256, pad=0.06)
    ico_path = os.path.join(ICONS, icon.ico")
    ico_master.save(ico_path, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(wrote icon.ico (multi-size)")

    
    print(DONE")

if __name__ == __main__":
    main()
