"""Redraw the Sift icon as SVG, render with librsvg, and build comparison PNGs.

Run: python3 /Users/maniksoomro/Documents/sift/assets/icon-draft/regenerate.py
Requires Python Pillow and rsvg-convert (Homebrew librsvg).
"""
from pathlib import Path
import math
import subprocess
from PIL import Image, ImageDraw, ImageFont

OUT = Path(__file__).resolve().parent
SOURCE = Path('/Users/maniksoomro/.t3/userdata/attachments/e5c716de-4dac-4dbf-8f5e-c52344d1d4b7-4c75759e-5dea-4906-b7e1-a944fd582bc9.png')
OUT.mkdir(parents=True, exist_ok=True)

defs = '''
<linearGradient id="silver" x1="0" x2="1">
 <stop offset="0" stop-color="#737575"/><stop offset=".055" stop-color="#b9bbbb"/>
 <stop offset=".19" stop-color="#eeeeed"/><stop offset=".29" stop-color="#fafaf8"/>
 <stop offset=".42" stop-color="#dadbd9"/><stop offset=".65" stop-color="#bdbfbd"/>
 <stop offset=".84" stop-color="#a0a2a0"/><stop offset=".96" stop-color="#747774"/>
 <stop offset="1" stop-color="#60635f"/>
</linearGradient>
<linearGradient id="faceShade" x1="0" y1="0" x2="0" y2="1">
 <stop stop-color="#fff" stop-opacity=".13"/><stop offset=".53" stop-color="#fff" stop-opacity="0"/>
 <stop offset="1" stop-color="#262a27" stop-opacity=".14"/>
</linearGradient>
<radialGradient id="top" cx=".35" cy=".28" r=".8">
 <stop stop-color="#e7e8e5"/><stop offset=".45" stop-color="#ced0cd"/>
 <stop offset=".83" stop-color="#aeb1ad"/><stop offset="1" stop-color="#8d918c"/>
</radialGradient>
<linearGradient id="rim" x1="0" y1="0" x2="0" y2="1">
 <stop stop-color="#fafbf8"/><stop offset=".45" stop-color="#c7c9c5"/>
 <stop offset="1" stop-color="#71756e"/>
</linearGradient>
<filter id="threadShadow" x="-30%" y="-20%" width="160%" height="150%" color-interpolation-filters="sRGB">
 <feGaussianBlur stdDeviation="4.5"/>
</filter>
<clipPath id="stackClip"><path d="M86 188 A426 116 0 0 1 938 188 V824 A426 116 0 0 1 86 824Z"/></clipPath>
'''

def tier(top, bottom):
    # Each tier has a curved front surface and a separate, fully elliptical top.
    body = f'M86 {top} A426 116 0 0 0 938 {top} V{bottom} A426 116 0 0 1 86 {bottom}Z'
    return f'''<g>
 <path d="{body}" fill="url(#silver)" stroke="#696e67" stroke-width="2"/>
 <path d="{body}" fill="url(#faceShade)"/>
 <path d="M89 {bottom-5} A423 114 0 0 0 935 {bottom-5}" fill="none" stroke="#f5f6f1" stroke-opacity=".55" stroke-width="3"/>
 <ellipse cx="512" cy="{top}" rx="426" ry="116" fill="url(#top)" stroke="url(#rim)" stroke-width="7"/>
 <path d="M94 {top-7} A418 108 0 0 1 930 {top-7}" fill="none" stroke="#ffffff" stroke-opacity=".44" stroke-width="2"/>
 <path d="M91 {top+9} A421 112 0 0 0 933 {top+9}" fill="none" stroke="#ffffff" stroke-opacity=".8" stroke-width="3"/>
</g>'''

# A 390 px wound wool ball in front of the untouched lower-right tier.
# Each wrap has its own groove, rounded body, highlight, and fine wool fibres.
def strand(d, width=14):
    return f'''<g fill="none" stroke-linecap="round" stroke-linejoin="round">
      <path d="{d}" stroke="#8f9087" stroke-width="{width+3}" opacity=".74"/>
      <path d="{d}" stroke="url(#woolStrand)" stroke-width="{width}"/>
      <path d="{d}" transform="translate(-1.5 -2.5)" stroke="#fffef4" stroke-width="{width*.26}" opacity=".85"/>
      <path d="{d}" transform="translate(2 2)" stroke="#d3d1c5" stroke-width=".8" opacity=".65"/>
    </g>'''

wraps=[]
# The underlying winding sweeps from lower-left to upper-right. The bow increases
# toward the silhouette, so these are strands around a volume, not flat stripes.
for i in range(-30,36):
    k=i*15
    d=f'M{540+k} 1005 C{520+k} 834 {650+k} 625 {872+k} 557'
    wraps.append(strand(d,13.5))

# Broad crossing winding: seven continuous parallel strands form a curved belt.
cross=[]
for i in range(9):
    k=i*14
    d=f'M{568+k*.42} {650-k*.57} C{690+k*.45} {650-k*.2} {797+k*.54} {846-k*.18} {907+k*.4} {935-k*.67} C{947+k*.4} {968-k*.67} 1000 1000 1050 1030'
    cross.append(strand(d,14))

# A separate return winding cups the right-hand side, with ends tucked beneath
# the crossing belt. This third direction makes the ball visibly wound.
side=[]
for i in range(7):
    k=i*14
    d=f'M{795+k*.54} {575+k*.18} C{1005+k*.06} {677+k*.2} {1030-k*.3} {833+k*.3} {850-k*.6} {1050-k*.32}'
    side.append(strand(d,13.5))

# Fine, irregular flyaways; a restrained edge treatment keeps the 64 px clear.
wisps=[]
for i in range(53):
    a=i*2*math.pi/53
    x=795+194*math.cos(a); y=790+191*math.sin(a)
    xx=x+(3+i%4)*math.cos(a); yy=y+(3+i%3)*math.sin(a)
    wisps.append(f'<path d="M{x:.1f} {y:.1f} Q{xx-2:.1f} {yy-2:.1f} {xx:.1f} {yy:.1f}" fill="none" stroke="#eeede2" stroke-width=".9" opacity=".7"/>')

tail='M650 916 C627 939 609 960 578 956 C560 954 551 946 541 948'
# Yarn ball size: scaled about a point near the icon's bottom-right so it stays in the corner.
YARN_SCALE, YARN_AX, YARN_AY = 0.72, 880, 978
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
<title>Sift — silver database with a white ball of knitting yarn</title>
<desc>Three unchanged silver database tiers. A cream-white wool ball overlaps the lower-right tier, with curved strands wound in three crossing directions, top-left highlights, soft contact shadows, and a short smooth frayed end.</desc>
<defs>{defs}
 <radialGradient id="wool" cx=".28" cy=".22" r=".85">
  <stop stop-color="#fffdf1"/><stop offset=".42" stop-color="#eeede3"/><stop offset=".78" stop-color="#d4d4cc"/><stop offset="1" stop-color="#a6aaa3"/>
 </radialGradient>
 <linearGradient id="woolStrand" x1="0" y1="0" x2=".75" y2="1">
  <stop stop-color="#fffdf2"/><stop offset=".48" stop-color="#efeee4"/><stop offset="1" stop-color="#c4c7bf"/>
 </linearGradient>
 <radialGradient id="woolLight" cx=".26" cy=".19" r=".83">
  <stop stop-color="#fff9de" stop-opacity=".3"/><stop offset=".42" stop-color="#fffdf4" stop-opacity=".08"/><stop offset=".72" stop-color="#555e56" stop-opacity=".02"/><stop offset="1" stop-color="#535c54" stop-opacity=".33"/>
 </radialGradient>
 <clipPath id="ballClip"><ellipse cx="795" cy="790" rx="194" ry="191"/></clipPath>
 <filter id="contact" x="-30%" y="-30%" width="160%" height="160%" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="11"/></filter>
 <filter id="ground" x="-30%" y="-150%" width="160%" height="400%" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="8"/></filter>
</defs>
{tier(640,824)}{tier(416,600)}{tier(188,372)}
<g transform="translate({YARN_AX} {YARN_AY}) scale({YARN_SCALE}) translate(-{YARN_AX} -{YARN_AY})">
<g clip-path="url(#stackClip)"><ellipse cx="797" cy="802" rx="207" ry="191" fill="#30362f" opacity=".42" filter="url(#contact)"/></g>
<ellipse cx="795" cy="977" rx="171" ry="14" fill="#252b27" opacity=".27" filter="url(#ground)"/>
<g fill="none" stroke-linecap="round">
 <path d="{tail}" transform="translate(1 5)" stroke="#333b32" stroke-width="17" opacity=".2" filter="url(#threadShadow)"/>
 <path d="{tail}" stroke="#969c91" stroke-width="14"/>
 <path d="{tail}" stroke="#e9e9dd" stroke-width="11"/>
 <path d="{tail}" transform="translate(0 -2)" stroke="#fffdf2" stroke-width="4"/>
 <path d="M549 948 Q539 942 532 946 M548 949 Q537 951 532 949 M546 950 Q539 955 534 953" stroke="#e9e7da" stroke-width="1.8"/>
</g>
<ellipse cx="795" cy="790" rx="194" ry="191" fill="url(#wool)" stroke="#838c80" stroke-width="4"/>
<g clip-path="url(#ballClip)">
 {''.join(wraps)}
 {''.join(side)}
 {''.join(cross)}
 <ellipse cx="795" cy="790" rx="194" ry="191" fill="url(#woolLight)"/>
</g>
{''.join(wisps)}
</g>
</svg>'''
(OUT/'icon.svg').write_text(svg)
render = subprocess.run(['rsvg-convert','--width','2048','--height','2048',str(OUT/'icon.svg')],check=True,capture_output=True)
import io
large = Image.open(io.BytesIO(render.stdout)).convert('RGBA').resize((1024,1024),Image.Resampling.LANCZOS)
large.save(OUT/'icon-1024.png')
original = Image.open(SOURCE).convert('RGBA')
large.resize(original.size,Image.Resampling.LANCZOS).save(OUT/'icon-small.png')
large.resize((64,64),Image.Resampling.LANCZOS).save(OUT/'icon-64.png')

preview=Image.new('RGB',(656,840),'#ededeb')
draw=ImageDraw.Draw(preview)
fontpath='/System/Library/Fonts/Supplemental/Arial.ttf'
font=ImageFont.truetype(fontpath,18)
smallfont=ImageFont.truetype(fontpath,14)
for row,(bg,fg,label) in enumerate([('#f1f0ec','#333531','LIGHT'),('#202327','#ebedee','DARK')]):
    y=row*420
    draw.rectangle((0,y,655,y+419),fill=bg)
    draw.text((28,y+14),label,font=smallfont,fill=fg)
    for col,(im,name) in enumerate([(original.resize((256,256),Image.Resampling.NEAREST),'Original · 39 × 39, enlarged'),(large.resize((256,256),Image.Resampling.LANCZOS),'Sift · ball of white yarn')]):
        x=28+col*316
        preview.paste(im,(x,y+49),im)
        draw.text((x,y+315),name,font=font,fill=fg)
    # Native-size checks on each background, beside the enlarged comparison.
    for x, size in [(344,64),(468,39)]:
        thumb=large.resize((size,size),Image.Resampling.LANCZOS)
        preview.paste(thumb,(x,y+343),thumb)
        draw.text((x+size+7,y+363),f'{size} px',font=smallfont,fill=fg)
preview.save(OUT/'preview.png')
print(f'Source: {original.size}; saved SVG, 1024 PNG, {original.width}px PNG, and preview to {OUT}')
