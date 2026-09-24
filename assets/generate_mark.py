#!/usr/bin/env python3
"""Generates the Tuxedo logo and the other concepts drawn for it.

lapel is the logo in use. Three concepts, each in the variants a project
needs:

- bowtie:   a solid bow tie, two curved wings pinched into a knot, with the
            pleats cut out of the fabric.
- brackets: HankoXml's `< >` chevrons turned inward, `>•<`: the same code
            brackets, tied into a bow tie by a knot.
- lapel:    a tuxedo front, the jacket as a field with the shirt's V, a bow
            tie and two studs.

Variants: `<name>.svg` in tuxedo black, `<name>-inverse.svg` in white for
dark backgrounds, `<name>-compact.svg` for 32px and below (a solid field
with the mark knocked out; `-compact-inverse` is the white one), and
`<name>-gem.svg` with a ruby red knot for the tuxedo-r gem.

Every shape is emitted as filled outlines, one <path> per colour, with no
stroke attributes, no <style> and no <mask>, so a file renders identically
through GitHub's SVG sanitizer, browser favicon pipelines and ICO
conversion. Holes are cut with fill-rule="evenodd", so two filled shapes in
one path must never overlap: an overlap XORs into a notch.

Usage:
    python3 assets/generate_mark.py assets/
"""
import math
import os
import sys

INK = "#141414"
PAPER = "#FAFAFA"
RUBY = "#CC342D"


# --- geometry --------------------------------------------------------------
def poly(pts):
    return "M" + "L".join("%.2f %.2f" % p for p in pts) + "Z"


def mirror_x(pts):
    return [(128 - x, y) for x, y in reversed(pts)]


def mirror_y(pts, cy=64):
    return [(x, 2 * cy - y) for x, y in reversed(pts)]


def circle(cx, cy, r, n=24):
    return poly([(cx + r * math.cos(2 * math.pi * i / n),
                  cy + r * math.sin(2 * math.pi * i / n)) for i in range(n)])


def quad(p0, c, p1, n=10):
    """Points along a quadratic Bezier, so a curved edge can live in a
    polygon and be mirrored with it."""
    return [((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * c[0] + t * t * p1[0],
             (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * c[1] + t * t * p1[1])
            for t in (i / n for i in range(1, n + 1))]


def _unit(a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    length = math.hypot(dx, dy)
    return dx / length, dy / length


def _intersect(p1, p2, p3, p4):
    (x1, y1), (x2, y2), (x3, y3), (x4, y4) = p1, p2, p3, p4
    den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
    if abs(den) < 1e-9:
        return p2
    a = x1 * y2 - y1 * x2
    b = x3 * y4 - y3 * x4
    return ((a * (x3 - x4) - (x1 - x2) * b) / den,
            (a * (y3 - y4) - (y1 - y2) * b) / den)


def _offset(pts, d):
    segs = []
    for a, b in zip(pts, pts[1:]):
        ux, uy = _unit(a, b)
        nx, ny = -uy, ux
        segs.append(((a[0] + nx * d, a[1] + ny * d), (b[0] + nx * d, b[1] + ny * d)))
    out = [segs[0][0]]
    for s1, s2 in zip(segs, segs[1:]):
        out.append(_intersect(s1[0], s1[1], s2[0], s2[1]))
    out.append(segs[-1][1])
    return out


def stroke(pts, w):
    """A polyline as a closed outline: square caps, miter joins. The same
    construction as HankoXml's chevrons."""
    ux, uy = _unit(pts[0], pts[1])
    pts = [(pts[0][0] - ux * w / 2, pts[0][1] - uy * w / 2)] + list(pts[1:])
    ux, uy = _unit(pts[-2], pts[-1])
    pts = list(pts[:-1]) + [(pts[-1][0] + ux * w / 2, pts[-1][1] + uy * w / 2)]
    return _offset(pts, w / 2) + list(reversed(_offset(pts, -w / 2)))


def rounded_square(r, lo=0.0, hi=128.0):
    k = 0.5523 * r  # a quarter circle as one cubic
    return (
        f"M{lo + r:.2f} {lo:.2f}H{hi - r:.2f}"
        f"C{hi - r + k:.2f} {lo:.2f} {hi:.2f} {lo + r - k:.2f} {hi:.2f} {lo + r:.2f}"
        f"V{hi - r:.2f}"
        f"C{hi:.2f} {hi - r + k:.2f} {hi - r + k:.2f} {hi:.2f} {hi - r:.2f} {hi:.2f}"
        f"H{lo + r:.2f}"
        f"C{lo + r - k:.2f} {hi:.2f} {lo:.2f} {hi - r + k:.2f} {lo:.2f} {hi - r:.2f}"
        f"V{lo + r:.2f}"
        f"C{lo:.2f} {lo + r - k:.2f} {lo + r - k:.2f} {lo:.2f} {lo + r:.2f} {lo:.2f}Z"
    )


FIELD = rounded_square(28)


def svg(paths):
    body = "".join(f'  <path fill="{c}" fill-rule="evenodd" d="{d}"/>\n' for c, d in paths)
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" '
            'role="img" aria-label="Tuxedo">\n'
            "  <title>Tuxedo</title>\n" + body + "</svg>\n")


# --- the ruby knot, shared by the gem variants --------------------------------
def ruby(cx=64, cy=64, s=1.0):
    """A cut gem seen side-on, table up: the Ruby logo's silhouette, with no
    facets, since facet lines close up at the size a gem page shows it."""
    pts = [(-9, -12), (9, -12), (15, -4), (0, 15), (-15, -4)]
    return poly([(cx + x * s, cy + y * s) for x, y in pts])


# --- A: bowtie -------------------------------------------------------------
# The left wing, from the top of the knot round to its bottom. The top and
# bottom edges bow outward and the outer edge dents in: that is what turns
# two triangles into cloth rather than an hourglass.
def _bowtie_wing(scale=1.0, dx=0.0):
    top = [(50, 56)] + quad((50, 56), (30, 36), (12, 32))
    outer = quad((12, 32), (24, 64), (12, 96))
    bottom = quad((12, 96), (30, 92), (50, 72))
    pts = top + outer[:-1] + bottom
    return [(64 + (x - 64) * scale + dx, 64 + (y - 64) * scale) for x, y in pts]


# Pleats: two thin wedges fanning from the knot toward the dent.
BOWTIE_PLEATS = [[(46, 60.5), (24, 49), (23.5, 52.5)],
                 [(46, 67.5), (23.5, 75.5), (24, 79)]]
BOWTIE_KNOT = [(54, 53), (74, 53), (76, 64), (74, 75), (54, 75), (52, 64)]


def bowtie_wings():
    w = _bowtie_wing()
    d = poly(w) + poly(mirror_x(w))
    for p in BOWTIE_PLEATS:
        d += poly(p) + poly(mirror_x(p))
    return d


def bowtie(color=INK):
    return svg([(color, bowtie_wings() + poly(BOWTIE_KNOT))])


def bowtie_compact(color=INK):
    # Wings and knot fused into one knockout: the gaps would close at 16px.
    w = [(x, y) for x, y in _bowtie_wing(0.78)]
    w = [(58, w[0][1])] + w[1:-1] + [(58, w[-1][1])]
    return svg([(color, FIELD + poly(w + mirror_x(w)))])


def bowtie_gem():
    return svg([(INK, bowtie_wings()), (RUBY, ruby(64, 63, 0.95))])


# --- B: brackets -------------------------------------------------------------
BR_W = 11.0
BR_L = [(20, 36), (48, 64), (20, 92)]


def brackets_shapes(knot=True):
    d = poly(stroke(BR_L, BR_W)) + poly(stroke(mirror_x(BR_L), BR_W))
    return d + (circle(64, 64, 8.5) if knot else "")


def brackets(color=INK):
    return svg([(color, brackets_shapes())])


def brackets_compact(color=INK):
    left = [(30, 42), (50, 64), (30, 86)]
    d = FIELD + poly(stroke(left, 11)) + poly(stroke(mirror_x(left), 11)) + circle(64, 64, 7)
    return svg([(color, d)])


def brackets_gem():
    return svg([(INK, brackets_shapes(knot=False)), (RUBY, ruby(64, 64, 0.62))])


# --- C: lapel ----------------------------------------------------------------
# The jacket is the field and the shirt is a hole in it; the tie and studs
# sit inside the hole, so evenodd fills them again. Nothing may cross the
# shirt's edge.
LAPEL_SHIRT = [(34, 18), (94, 18), (64, 112)]
LAPEL_WING = [(61, 33), (46, 26), (48.5, 36), (46, 46), (61, 39)]
LAPEL_KNOT = [(62.5, 32), (65.5, 32), (66.5, 36), (65.5, 40), (62.5, 40), (61.5, 36)]


def lapel_parts():
    tie = poly(LAPEL_WING) + poly(mirror_x(LAPEL_WING))
    studs = circle(64, 62, 3.4) + circle(64, 80, 3.4)
    knot = poly(LAPEL_KNOT)
    return tie, studs, knot


def lapel(color=INK):
    tie, studs, knot = lapel_parts()
    return svg([(color, FIELD + poly(LAPEL_SHIRT) + tie + knot + studs)])


def lapel_gem():
    tie, studs, knot = lapel_parts()
    return svg([(INK, FIELD + poly(LAPEL_SHIRT) + tie + studs), (RUBY, knot)])


VARIANTS = {
    "tuxedo-bowtie.svg": bowtie,
    "tuxedo-bowtie-inverse.svg": lambda: bowtie(PAPER),
    "tuxedo-bowtie-compact.svg": bowtie_compact,
    "tuxedo-bowtie-compact-inverse.svg": lambda: bowtie_compact(PAPER),
    "tuxedo-bowtie-gem.svg": bowtie_gem,
    "tuxedo-brackets.svg": brackets,
    "tuxedo-brackets-inverse.svg": lambda: brackets(PAPER),
    "tuxedo-brackets-compact.svg": brackets_compact,
    "tuxedo-brackets-compact-inverse.svg": lambda: brackets_compact(PAPER),
    "tuxedo-brackets-gem.svg": brackets_gem,
    "tuxedo-lapel.svg": lapel,
    "tuxedo-lapel-inverse.svg": lambda: lapel(PAPER),
    "tuxedo-lapel-compact.svg": lapel,
    "tuxedo-lapel-compact-inverse.svg": lambda: lapel(PAPER),
    "tuxedo-lapel-gem.svg": lapel_gem,
}

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(__file__)
    for name, make in VARIANTS.items():
        with open(os.path.join(out, name), "w") as fh:
            fh.write(make())
        print("wrote", os.path.join(out, name))
