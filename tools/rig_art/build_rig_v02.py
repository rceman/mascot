"""Build mascot rig v0.2 attachments + rig.json from assets/mascot.png.

Pipeline (see docs/MASCOT_RIG_V0.2_ARCHITECTURE.md for the rationale):

1. Silhouette S = canonical alpha >= 0.5. Pixels farther than OUTLINE_RADIUS
   from the background form the fill interior F0; the band S \\ F0 is the
   canonical external stroke and is *not* stored in any attachment - the
   runtime regenerates it by dilating the composited fill alpha.
2. Visible colour pixels in F0 are assigned to parts with authored ownership
   polygons. Black line pixels in F0 are classified automatically:
     * contour  - the line separates two parts: owned by the higher-z part,
                  the lower part's fill is continued underneath it;
     * detail   - both sides belong to one part: the part's fill continues
                  underneath (eyes, nose, ear interior, rims).
   Anti-aliased line pixels are un-mixed into (fill colour, line alpha).
3. Hidden geometry: authored extension shapes are clipped to the region covered
   by higher-z parts at rest (so the rest pose is unchanged) and coloured by
   nearest-visible-colour propagation. The hidden boundary of every part gets
   an automatic contour stroke of the canonical width, also hidden at rest.
4. Protrusions the runtime outline cannot regenerate (whisker tips, acute
   corners) are kept as line art.
5. Fully reconstructed parts (ear_far, arm_far) are generated and verified to
   be completely occluded at rest.

Outputs: assets/mascot/rig-v0.2/{rig.json, parts/*.png} and, with --debug,
diagnostic images in the given directory.
"""
import argparse
import json
import math
import os
import sys

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage as ndi

sys.path.insert(0, os.path.dirname(__file__))
import decomposition as D  # noqa: E402

PALETTE = {
    "black": (0, 0, 0),
    "orange_light": (250, 157, 60),
    "orange_dark": (222, 126, 34),
    "white": (250, 250, 250),
    "grey_light": (216, 216, 216),
    "grey_mid": (162, 162, 162),
}
PURE_TOL = 18.0
UNMIX_RESIDUAL = 26.0
H = W = None


def edt_to(mask, indices=False):
    """Distance from every pixel to the nearest pixel of `mask`."""
    if indices:
        d, idx = ndi.distance_transform_edt(~mask, return_indices=True)
        return d, idx
    return ndi.distance_transform_edt(~mask)


def raster(shapes, size):
    img = Image.new("L", size, 0)
    dr = ImageDraw.Draw(img)
    for shape in shapes:
        kind, g = shape[0], shape[1]
        if kind == "poly":
            dr.polygon([tuple(p) for p in g], fill=255)
        elif kind == "ellipse":
            cx, cy, rx, ry = g
            dr.ellipse((cx - rx, cy - ry, cx + rx, cy + ry), fill=255)
        elif kind == "rrect":
            x0, y0, x1, y1, r = g
            dr.rounded_rectangle((x0, y0, x1, y1), radius=r, fill=255)
        elif kind == "capsule":
            (x0, y0), (x1, y1), r = g
            dr.line((x0, y0, x1, y1), fill=255, width=int(2 * r))
            dr.ellipse((x0 - r, y0 - r, x0 + r, y0 + r), fill=255)
            dr.ellipse((x1 - r, y1 - r, x1 + r, y1 + r), fill=255)
        else:
            raise ValueError(kind)
    return np.array(img) > 127


def band_alpha(fill, radius):
    """Anti-aliased stroke of width `radius` hugging the outside of `fill`."""
    d = edt_to(fill)
    a = np.clip(radius + 0.5 - (d - 0.5), 0.0, 1.0)
    a[fill] = 0.0
    return a


def main():
    global H, W
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..")))
    ap.add_argument("--debug", default=None)
    args = ap.parse_args()
    repo = args.repo
    out_dir = os.path.join(repo, "assets", "mascot", "rig-v0.2")
    parts_dir = os.path.join(out_dir, "parts")
    os.makedirs(parts_dir, exist_ok=True)
    dbg = args.debug
    if dbg:
        os.makedirs(dbg, exist_ok=True)

    src = np.array(Image.open(os.path.join(repo, D.SOURCE)).convert("RGBA")).astype(np.float32)
    H, W = src.shape[:2]
    assert (W, H) == D.CANVAS
    rgb = src[..., :3]
    alpha = src[..., 3] / 255.0
    R = D.OUTLINE_RADIUS

    # --- 1. silhouette, interior, classes -----------------------------------
    S = alpha >= 0.5
    d_bg = ndi.distance_transform_edt(S)
    F0 = d_bg > R
    F0_soft = np.clip(d_bg - R + 0.5, 0.0, 1.0)
    names = list(PALETTE)
    pal = np.array([PALETTE[n] for n in names], np.float32)
    dist = np.sqrt(((rgb[..., None, :] - pal[None, None]) ** 2).sum(-1))
    cls = dist.argmin(-1)
    cls_d = dist.min(-1)
    is_black = cls == 0
    black = is_black & F0
    color = (~is_black) & F0
    # AA ramps from yellow to black pass through dark orange, so a pure
    # reference pixel must also be clear of line art.
    pure = color & (cls_d < PURE_TOL) & (edt_to(is_black & S) > 2.5)

    # external stroke thickness report
    nb = ndi.binary_dilation(is_black & S, np.ones((3, 3), bool))
    edge_vals = d_bg[(~is_black) & S & nb & (d_bg < 60)]
    thickness = dict(median=float(np.median(edge_vals)), p10=float(np.percentile(edge_vals, 10)),
                     p90=float(np.percentile(edge_vals, 90)))

    # --- line alpha un-mixing ------------------------------------------------
    _, idx = edt_to(pure, indices=True)
    f_ref = rgb[idx[0], idx[1]]
    ff = (f_ref * f_ref).sum(-1) + 1e-6
    a_line = 1.0 - (rgb * f_ref).sum(-1) / ff
    a_line = np.clip(a_line, 0.0, 1.0)
    resid = np.sqrt(((rgb - f_ref * (1.0 - a_line)[..., None]) ** 2).sum(-1))
    a_line[(resid > UNMIX_RESIDUAL) & ~is_black] = 0.0
    a_line[pure] = 0.0
    a_line[a_line < 0.03] = 0.0
    unmixed = np.where((a_line > 0)[..., None], f_ref, rgb)

    # --- 2. colour ownership -------------------------------------------------
    part_by_id = {p["id"]: p for p in D.PARTS}
    pids = [p["id"] for p in D.PARTS]
    z = {p["id"]: p["z"] for p in D.PARTS}
    ucls = np.sqrt(((unmixed[..., None, :] - pal[None, None]) ** 2).sum(-1)).argmin(-1)
    owner_color = np.full((H, W), -1, np.int32)
    for pid in D.OWN_PRIORITY:
        k = pids.index(pid)
        allowed = np.isin(ucls, [names.index(c) for c in part_by_id[pid]["colors"]])
        m = raster(part_by_id[pid]["own"], (W, H)) & color & allowed & (owner_color < 0)
        owner_color[m] = k
    # islands (AA specks that slipped past the colour filter) join their surroundings
    islands = 0
    for k in range(len(pids)):
        lab, n = ndi.label(owner_color == k)
        if n == 0:
            continue
        sizes = ndi.sum(owner_color == k, lab, range(1, n + 1))
        small = np.isin(lab, [i + 1 for i, s in enumerate(sizes) if s < 300])
        islands += int(small.sum())
        owner_color[small] = -1
    stray = color & (owner_color < 0)
    if stray.any():
        _, sidx = edt_to(owner_color >= 0, indices=True)
        owner_color[stray] = owner_color[sidx[0][stray], sidx[1][stray]]
    assert (owner_color[color] >= 0).all()
    color_k = [owner_color == k for k in range(len(pids))]

    # --- line classification --------------------------------------------------
    dists = np.stack([edt_to(c) if c.any() else np.full((H, W), 1e9, np.float32) for c in color_k]).astype(np.float32)
    order = np.argsort(dists, axis=0)
    p1 = order[0]
    p2 = order[1]
    d1 = np.take_along_axis(dists, order[0:1], 0)[0]
    d2 = np.take_along_axis(dists, order[1:2], 0)[0]
    zarr = np.array([z[p] for p in pids])
    contour = black & (d2 < D.CONTOUR_DETECT)
    merged = contour & (d1 + d2 > D.MERGED_BAND)
    single = contour & ~merged
    # single stroke between the two nearest parts: higher owns it, lower fills beneath
    hi = np.where(zarr[p1] >= zarr[p2], p1, p2)
    lo = np.where(zarr[p1] >= zarr[p2], p2, p1)
    line_owner = np.where(single, hi, p1)
    line_under = np.where(single, lo, p1)
    # merged double stroke: each side keeps its own half, nothing beneath
    line_under[merged] = -1
    line_also = {}
    d_color = edt_to(color)
    for ov in D.LINE_OVERRIDES:
        m = raster([("poly", ov["poly"])], (W, H)) & black
        if ov.get("drop"):
            # external-stroke mass deeper than R from the background: the runtime
            # outline (dilation of the fill by R) regenerates it wherever it is
            # within R of colour, so it must not exist as static line art
            m &= d_color <= R - 1.0
            line_owner[m] = -1
            line_under[m] = -1
            continue
        if ov.get("split_reach"):
            # merged stroke of two parts: nearest part owns each pixel (rest is
            # fully covered) and each part also carries the band within `reach`
            # of its own colour, i.e. a normal-width contour that follows its
            # own edge and never the other part's shape
            ka, kb = pids.index(ov["owner"]), pids.index(ov["also"])
            reach = ov["split_reach"]
            a_near = dists[ka] <= dists[kb]
            line_owner[m] = np.where(a_near[m], ka, kb)
            line_under[m] = -1
            for k, other in ((ka, ~a_near), (kb, a_near)):
                extra = m & other & (dists[k] <= reach)
                line_also[k] = line_also.get(k, np.zeros((H, W), bool)) | extra
            continue
        line_owner[m] = pids.index(ov["owner"])
        if ov.get("under") == "none":
            line_under[m] = -1
        elif ov.get("under"):
            line_under[m] = pids.index(ov["under"])
        if ov.get("also"):
            # shared stroke: both parts carry a copy (each needs its own edge
            # once they separate); nothing fills beneath. Pixels farther than
            # `owner_reach` from the owner's colour belong to `also` alone.
            k2 = pids.index(ov["also"])
            far = m & (dists[pids.index(ov["owner"])] > ov.get("owner_reach", 1e9))
            line_owner[far] = k2
            line_also[k2] = line_also.get(k2, np.zeros((H, W), bool)) | (m & ~far)
    line_owner[~black] = -1
    line_under[~black] = -1

    # A part may only fill beneath another part's stroke inside its own plausible
    # extent; otherwise e.g. whisker-shaped arm fill would travel with the arm.
    extent = []
    for k, pid in enumerate(pids):
        p = part_by_id[pid]
        e = ndi.binary_dilation(color_k[k], iterations=6)
        if p.get("own_is_extent", True):
            e |= raster(p["own"], (W, H))
        if p["ext"]:
            e |= raster(p["ext"], (W, H))
        extent.append(e)
    extent = np.stack(extent)
    und = line_under >= 0
    bad = und & ~np.take_along_axis(extent, np.maximum(line_under, 0)[None], 0)[0]
    if bad.any():
        own_z = zarr[np.maximum(line_owner, 0)]
        cand = np.where(extent & (zarr[:, None, None] < own_z[None]), dists, np.inf)
        alt = cand.argmin(0)
        ok = np.isfinite(cand.min(0))
        line_under[bad] = np.where(ok[bad], alt[bad], -1)
    if dbg:
        print("line_under reassigned:", int(bad.sum()))

    # AA ramp alpha on colour pixels -> owner of nearest black pixel
    black_all = is_black & S
    _, bidx = edt_to(black_all, indices=True)
    nb_in_band = ~F0[bidx[0], bidx[1]]
    ramp = color & (a_line > 0)
    ramp_drop = ramp & nb_in_band
    a_line[ramp_drop] = 0.0
    unmixed[ramp_drop] = f_ref[ramp_drop]
    ramp = color & (a_line > 0)
    # c/(1-a) amplifies noise; the flat reference colour avoids light ghost
    # rims once the stroke that darkened the ramp moves away
    unmixed[ramp] = f_ref[ramp]
    ramp_owner = np.full((H, W), -1, np.int32)
    ramp_owner[ramp] = line_owner[bidx[0][ramp], bidx[1][ramp]]
    if dbg:
        rng = np.random.default_rng(3)
        cols = (rng.random((len(pids) + 1, 3)) * 200 + 55).astype(np.uint8)
        vis = np.zeros((H, W, 3), np.uint8) + 40
        for k in range(len(pids)):
            vis[color_k[k]] = cols[k]
        lo_vis = vis.copy()
        for k in range(len(pids)):
            vis[(line_owner == k)] = cols[k] // 3
            lo_vis[(line_under == k)] = cols[k] // 2
        Image.fromarray(vis).save(os.path.join(dbg, "own_lines.png"))
        lv = np.zeros((H, W, 3), np.uint8) + 255
        lv[S] = 225
        for k in range(len(pids)):
            lv[(line_owner == k)] = cols[k]
        Image.fromarray(lv).save(os.path.join(dbg, "line_owner.png"))
        Image.fromarray(lo_vis).save(os.path.join(dbg, "own_under.png"))
        with open(os.path.join(dbg, "legend.txt"), "w") as f:
            for k, pid in enumerate(pids):
                f.write(f"{pid} {tuple(int(c) for c in cols[k])}\n")

    # --- 4. protrusions the runtime outline cannot regenerate ---------------
    opened = edt_to(F0) <= R + 0.5
    keep = S & ~ndi.binary_dilation(opened, iterations=2)
    lab, n = ndi.label(keep)
    sizes = ndi.sum(keep, lab, range(1, n + 1))
    keep_big = np.isin(lab, [i + 1 for i, s in enumerate(sizes) if s >= 40])
    # kept pieces reach 3 px into the runtime-outline zone so their AA edge sits
    # on solid black instead of leaving a hairline seam against the outline's AA
    keep_aa = (alpha > 0) & ~(edt_to(F0) <= R - 3.0) & ndi.binary_dilation(keep_big, iterations=6)
    keep_owner = p1.copy()
    for ov in D.LINE_OVERRIDES:
        if ov.get("drop"):
            continue
        m = raster([("poly", ov["poly"])], (W, H))
        keep_owner[m] = pids.index(ov["owner"])
    # whisker tips on the right side belong to the head
    keep_owner[keep_aa] = np.where(dists[pids.index("head")][keep_aa] < 90, pids.index("head"), keep_owner[keep_aa])

    # --- 3. fills, hidden extensions, hidden contours -------------------------
    fills = {}
    lines = {}
    covered = np.zeros((H, W), bool)
    report = {}
    for zval in sorted(set(zarr.tolist()), reverse=True):
        layer_cov = np.zeros((H, W), bool)
        cov_er = ndi.binary_closing(covered, iterations=2) & F0
        for k, pid in enumerate(pids):
            if z[pid] != zval:
                continue
            p = part_by_id[pid]
            base_fill = color_k[k] | (line_under == k)
            authored = raster(p["ext"], (W, H)) if p["ext"] else np.zeros((H, W), bool)
            ext = authored & cov_er & ~base_fill
            clip_loss = authored & ~cov_er & ~base_fill & F0 & color & ~color_k[k]
            fill = base_fill | ext
            # colours
            allowed = np.isin(ucls, [names.index(c) for c in p["colors"]])
            pk = pure & color_k[k] & allowed
            if not pk.any():
                pk = color_k[k]
            _, pidx = edt_to(pk, indices=True)
            # hidden geometry takes the flat palette colour of the nearest
            # visible pixel (avoids propagating source colour noise as streaks)
            col = pal[ucls[pidx[0], pidx[1]]]
            for shape in p["ext"]:
                if len(shape) > 2 and shape[2]:
                    sm = raster([shape], (W, H)) & ext
                    col[sm] = PALETTE[shape[2]]
            col = np.where(color_k[k][..., None], unmixed, col)
            fa = fill.astype(np.float32)
            fa = np.where(color_k[k] | (line_under == k), fa * F0_soft, fa)
            # line art: owned lines + AA ramps + keep + hidden contour band
            la = np.zeros((H, W), np.float32)
            own = (line_owner == k)
            la[own] = np.maximum(a_line[own], 0.0)
            la[black & own] = np.maximum(la[black & own], a_line[black & own])
            if k in line_also:
                la[line_also[k]] = np.maximum(la[line_also[k]], a_line[line_also[k]])
            rm = ramp_owner == k
            la[rm] = np.maximum(la[rm], a_line[rm])
            km = keep_aa & (keep_owner == k)
            la[km] = np.maximum(la[km], alpha[km])
            hb = band_alpha(fill, R) * cov_er
            if p.get("no_contour"):
                hb[raster(p["no_contour"], (W, H))] = 0.0
            la = np.maximum(la, hb)
            fills[pid] = (fa, col)
            lines[pid] = la
            layer_cov |= (fa > 0.5) | (la > 0.5)
            report[pid] = dict(visible_px=int(color_k[k].sum()), ext_px=int(ext.sum()),
                               ext_clipped_px=int(clip_loss.sum()),
                               hidden_contour_px=int((hb > 0.5).sum()),
                               line_px=int((la > 0.5).sum()))
            if dbg and clip_loss.any():
                ys, xs = np.nonzero(clip_loss)
                print(f"clip {pid}: {int(clip_loss.sum())} px in x {xs.min()}-{xs.max()} y {ys.min()}-{ys.max()}")
        covered |= layer_cov

    # --- feather cut edges that have no line (neck, shoulder, hip) -------------
    # The upper part's alpha ramps to 0 over FEATHER px where it meets a lower
    # part's visible colour directly; the lower part continues beneath with the
    # same colour, so the rest pose is unchanged but motion shows no hard seam.
    for pid in D.FEATHER_PARTS:
        k = pids.index(pid)
        fa, col = fills[pid]
        lower_vis = np.zeros((H, W), bool)
        for j, q in enumerate(pids):
            if z[q] < z[pid]:
                lower_vis |= color_k[j]
        d = edt_to(lower_vis)
        # never near the silhouette: a feathered edge that rotates past the lower
        # part's boundary would let the runtime outline show through as a smudge
        zone = (fa > 0) & (d < D.FEATHER) & color_k[k] & (d_bg > R + 24.0)
        # composite of everything beneath (fills and lines, incl. hidden contours)
        beneath = np.zeros((H, W, 3), np.float32)
        b_a = np.zeros((H, W), np.float32)
        for q in sorted(fills, key=lambda q: z[q]):
            if z[q] >= z[pid]:
                continue
            qa, qc = fills[q]
            beneath = qc * qa[..., None] + beneath * (1 - qa[..., None])
            b_a = qa + b_a * (1 - qa)
            ql = lines[q][..., None]
            beneath = beneath * (1 - ql)
            b_a = lines[q] + b_a * (1 - lines[q])
        match = zone & (b_a > 0.99) & (np.abs(beneath - col).max(-1) < 40)
        w = np.clip(d / D.FEATHER, 0.0, 1.0)
        fa = np.where(zone & match, fa * w, fa)
        fills[pid] = (fa, col)
        report[pid]["feathered_px"] = int((zone & match).sum())

    # --- 5. reconstructed parts ----------------------------------------------
    recon_meta = {}
    cov_all = ndi.binary_erosion(covered, iterations=2)

    def covered_by_higher(zv):
        c = np.zeros((H, W), bool)
        for pid2 in fills:
            if z.get(pid2, 0) > zv:
                c |= fills[pid2][0] > 0.99
                c |= lines[pid2] > 0.99
        return ndi.binary_erosion(c, iterations=2)

    # ear_far: mirrored, scaled copy of ear_near, slid outward until just hidden
    spec = D.RECONSTRUCTED["ear_far"]
    ef_fa, ef_col = fills["ear_near"]
    ef_la = lines["ear_near"]
    ys, xs = np.nonzero((ef_fa > 0) | (ef_la > 0))
    y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    crop_fa = ef_fa[y0:y1, x0:x1][:, ::-1]
    crop_col = ef_col[y0:y1, x0:x1][:, ::-1]
    crop_la = ef_la[y0:y1, x0:x1][:, ::-1]
    sc = spec["scale"]
    nh, nw = int(round((y1 - y0) * sc)), int(round((x1 - x0) * sc))

    def rs(a, mode=Image.BILINEAR):
        return np.array(Image.fromarray(a.astype(np.float32)).resize((nw, nh), mode))

    s_fa = np.clip(rs(crop_fa), 0, 1)
    s_la = np.clip(rs(crop_la), 0, 1)
    s_col = np.stack([rs(crop_col[..., c]) for c in range(3)], -1)
    cov_ear = covered_by_higher(spec["z"])
    if dbg:
        Image.fromarray((cov_ear * 255).astype(np.uint8)).save(os.path.join(dbg, "cov_ear.png"))
        for pid in ("head", "eye_left"):
            Image.fromarray((fills[pid][0] * 255).astype(np.uint8)).save(os.path.join(dbg, f"fa_{pid}.png"))
            Image.fromarray((lines[pid] * 255).astype(np.uint8)).save(os.path.join(dbg, f"la_{pid}.png"))
    head_c = np.array([735.0, 380.0])
    direction = np.array(spec["centre"], np.float64) - head_c
    direction /= np.linalg.norm(direction)
    best = None
    for t in np.arange(0, 260, 2):
        c = head_c + direction * t
        ox, oy = int(round(c[0] - nw / 2)), int(round(c[1] - nh / 2))
        occ = (s_fa > 0.01) | (s_la > 0.01)
        sub = cov_ear[oy:oy + nh, ox:ox + nw]
        if sub.shape != occ.shape:
            break
        if (occ & ~sub).any():
            if dbg:
                print("ear_far stop t=", t, "uncovered", int((occ & ~sub).sum()), "box", ox, oy, nw, nh)
            break
        best = (ox, oy)
    assert best, "ear_far cannot be hidden"
    ox, oy = best
    fa = np.zeros((H, W), np.float32)
    la = np.zeros((H, W), np.float32)
    col = np.zeros((H, W, 3), np.float32)
    fa[oy:oy + nh, ox:ox + nw] = s_fa
    la[oy:oy + nh, ox:ox + nw] = s_la
    col[oy:oy + nh, ox:ox + nw] = s_col
    fills["ear_far"] = (fa, col)
    lines["ear_far"] = la
    z["ear_far"] = spec["z"]
    # pivot: near-ear pivot mirrored within the ear box
    ne_piv = dict((b[0], b[2]) for b in D.BONES)["ear_near"]
    rel_x = (ne_piv[0] - x0) / (x1 - x0)
    rel_y = (ne_piv[1] - y0) / (y1 - y0)
    ear_far_pivot = (round(ox + (1 - rel_x) * nw), round(oy + rel_y * nh))
    recon_meta["ear_far"] = dict(method="mirrored 0.9x copy of ear_near incl. reconstruction; "
                                        "slid outward from head centre until just fully occluded",
                                 offset=[ox, oy], pivot=list(ear_far_pivot))

    # arm_far: capsule forearm + paw, dark orange, auto contour, must be occluded
    spec = D.RECONSTRUCTED["arm_far"]
    cov_arm = covered_by_higher(spec["z"])
    piv = dict((b[0], b[2]) for b in D.BONES)
    sh, pw = np.array(piv["arm_far_upper"], float), np.array(piv["paw_far"], float)
    for rad in (56, 52, 48, 44, 40, 36):
        shape = raster([("capsule", (tuple(sh), tuple(pw), rad))], (W, H))
        shape_l = band_alpha(shape, R)
        occ = shape | (shape_l > 0.01)
        if not (occ & ~cov_arm).any():
            break
        if dbg:
            ys, xs = np.nonzero(occ & ~cov_arm)
            print("arm_far r", rad, "uncovered", len(ys), "x", xs.min(), xs.max(), "y", ys.min(), ys.max())
    else:
        raise SystemExit("arm_far cannot be hidden")
    fa = shape.astype(np.float32)
    col = np.zeros((H, W, 3), np.float32)
    col[:] = PALETTE["orange_dark"]
    fills["arm_far"] = (fa, col)
    lines["arm_far"] = shape_l
    z["arm_far"] = spec["z"]
    recon_meta["arm_far"] = dict(method="procedural capsule (shoulder->paw), flat body colour, "
                                        "auto contour; fully occluded by lid/lid_inner/arm_near/head",
                                 radius=rad)

    # --- verification: rest composite -----------------------------------------
    all_ids = sorted(fills, key=lambda p: z[p])
    comp = np.zeros((H, W, 3), np.float32)
    comp_a = np.zeros((H, W), np.float32)
    mask = np.zeros((H, W), np.float32)
    for pid in all_ids:
        fa, col = fills[pid]
        comp = col * fa[..., None] + comp * (1 - fa[..., None])
        comp_a = fa + comp_a * (1 - fa)
        mask = fa + mask * (1 - fa)
        la = lines[pid]
        comp = comp * (1 - la[..., None])
        comp_a = la + comp_a * (1 - la)
    d_out = edt_to(mask > 0.5)
    outline_a = np.clip(R + 0.5 - (d_out - 0.5), 0, 1)
    outline_a = np.maximum(outline_a, mask)
    final_a = comp_a + outline_a * (1 - comp_a)
    final_rgb = comp  # outline is black: contributes 0 colour
    final = np.dstack([np.where(final_a[..., None] > 0, final_rgb / np.maximum(final_a, 1e-6)[..., None], 0),
                       final_a * 255]).clip(0, 255).astype(np.uint8)
    diff = np.abs(final.astype(np.int32) - np.dstack([rgb, alpha * 255]).astype(np.int32))
    prem_final = final[..., :3].astype(np.float32) * (final[..., 3:4] / 255.0)
    prem_src = rgb * alpha[..., None]
    pdiff = np.abs(prem_final - prem_src).max(-1)
    uncovered = F0 & (mask < 0.5)
    stats = dict(rest_pdiff_mean=float(pdiff.mean()), rest_pdiff_gt32_px=int((pdiff > 32).sum()),
                 rest_pdiff_gt96_px=int((pdiff > 96).sum()), alpha_diff_gt64_px=int((diff[..., 3] > 64).sum()),
                 uncovered_interior_px=int(uncovered.sum()))

    # --- export ----------------------------------------------------------------
    bones = {b[0]: dict(parent=b[1], pivot=b[2], rng=b[3], note=b[4]) for b in D.BONES}
    bones["ear_far"]["pivot"] = ear_far_pivot
    attach = []
    part_bone = {p["id"]: p["bone"] for p in D.PARTS}
    part_bone["ear_far"] = D.RECONSTRUCTED["ear_far"]["bone"]
    part_bone["arm_far"] = D.RECONSTRUCTED["arm_far"]["bone"]
    for pid in all_ids:
        fa, col = fills[pid]
        la = lines[pid]
        occ = (fa > 0.002) | (la > 0.002)
        ys, xs = np.nonzero(occ)
        y0, y1 = max(ys.min() - 2, 0), min(ys.max() + 3, H)
        x0, x1 = max(xs.min() - 2, 0), min(xs.max() + 3, W)
        fimg = np.dstack([col[y0:y1, x0:x1], fa[y0:y1, x0:x1, None] * 255]).clip(0, 255).astype(np.uint8)
        fimg[fimg[..., 3] == 0, :3] = 0
        limg = np.zeros((y1 - y0, x1 - x0, 4), np.uint8)
        limg[..., 3] = (la[y0:y1, x0:x1] * 255).round().clip(0, 255).astype(np.uint8)
        Image.fromarray(fimg, "RGBA").save(os.path.join(parts_dir, f"{pid}.fill.png"), optimize=True)
        Image.fromarray(limg, "RGBA").save(os.path.join(parts_dir, f"{pid}.line.png"), optimize=True)
        bone = part_bone[pid]
        bp = bones[bone]["pivot"]
        visible_rest = occ & ~ndi.binary_erosion(covered_by_higher(z[pid]), iterations=0) if pid in ("ear_far", "arm_far") else None
        for role, suffix in (("fill", "fill"), ("line", "line")):
            attach.append(dict(
                id=f"{pid}.{suffix}", part=pid, kind="sprite", role=role, bone=bone,
                image=f"parts/{pid}.{suffix}.png",
                canvas_rect=[int(x0), int(y0), int(x1 - x0), int(y1 - y0)],
                origin=[float(x0 - bp[0]), float(y0 - bp[1])],
                z=int(z[pid]) * 2 + (1 if role == "line" else 0),
                visible=True, opacity=1.0,
                reconstructed=pid in recon_meta,
            ))

    # world rest pivots -> parent-local translations
    bone_list = []
    for b in D.BONES:
        bid = b[0]
        piv_w = bones[bid]["pivot"]
        par = bones[bid]["parent"]
        par_w = bones[par]["pivot"] if par else (0, 0)
        bone_list.append(dict(
            id=bid, parent=par,
            rest=dict(x=float(piv_w[0] - par_w[0]), y=float(piv_w[1] - par_w[1]),
                      rotation_deg=0.0, scale_x=1.0, scale_y=1.0),
            rest_world_pivot=[float(piv_w[0]), float(piv_w[1])],
            safe_rotation_deg=list(bones[bid]["rng"]),
            mirror="inherit",
            note=bones[bid]["note"],
        ))

    rig = dict(
        format="mascot-rig", version="0.2.0",
        source=dict(asset=D.SOURCE, width=W, height=H),
        coordinate_system=dict(
            canvas="canonical canvas pixels of source asset; origin top-left; +x right; +y down",
            bone_local="parent-local pixels; child rest translation = child pivot - parent pivot (canvas axes at rest)",
            rotation="degrees; positive = clockwise on screen (y-down)",
            composition="world = parent_world * T(x,y) * R(rotation) * S(scale_x,scale_y); bone origin == rotation pivot",
            attachment_origin="pixel offset of the sprite's top-left corner in the owning bone's local space",
            pivots="absolute pixels (not normalised)",
            mirror="whole-rig horizontal mirror = root scale_x -1 about the root pivot x",
        ),
        outline=dict(model="runtime: dilate(composited fill alpha, radius) drawn black beneath colour",
                     radius_canvas_px=R, canonical_stroke_measured=thickness,
                     line_art="role=line sprites are composited in z order but excluded from the outline mask"),
        shadow=dict(model="runtime: derived from composited silhouette; no baked shadow layers"),
        bones=bone_list,
        attachments=attach,
        reconstructed=recon_meta,
        build_stats=dict(parts=report, rest=stats),
    )
    guards_path = os.path.join(os.path.dirname(__file__), "guards_v02.json")
    if os.path.exists(guards_path):
        with open(guards_path) as f:
            guards = json.load(f)
        for g in guards:
            # authored in canvas pixels; stored in the guard bone's local space
            # (all rest rotations are 0 and scales 1, so local = canvas - pivot)
            piv = bones[g["bone"]]["pivot"]
            c = g.pop("center_canvas")
            g["center"] = [float(c[0] - piv[0]), float(c[1] - piv[1])]
            g["center_canvas_rest"] = [float(c[0]), float(c[1])]
        rig["joint_guards"] = guards
    with open(os.path.join(out_dir, "rig.json"), "w", newline="\n") as f:
        json.dump(rig, f, indent=2)
        f.write("\n")
    print(json.dumps(dict(thickness=thickness, rest=stats, recon=recon_meta), indent=1))

    if dbg:
        Image.fromarray(final, "RGBA").save(os.path.join(dbg, "py_rest.png"))
        Image.fromarray((np.clip(pdiff * 2, 0, 255)).astype(np.uint8)).save(os.path.join(dbg, "py_rest_diff.png"))
        for pid in all_ids:
            fa, col = fills[pid]
            la = lines[pid]
            img = np.zeros((H, W, 3), np.float32) + [120, 200, 255]
            img = col * fa[..., None] + img * (1 - fa[..., None])
            img = img * (1 - la[..., None])
            Image.fromarray(img.clip(0, 255).astype(np.uint8)).save(os.path.join(dbg, f"part_{pid}.png"))


if __name__ == "__main__":
    main()
