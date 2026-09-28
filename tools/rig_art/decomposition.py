"""Authored decomposition spec for mascot rig v0.2.

All coordinates are canonical canvas pixels of assets/mascot.png
(1254 x 1254, origin top-left, +x right, +y down).

This file is data only; build_rig_v02.py implements the algorithm.
"""

CANVAS = (1254, 1254)
SOURCE = "assets/mascot.png"

# Canonical external stroke thickness measured from the source (see
# measure_outline in build_rig_v02.py): first fill pixel lies 34-36 px from the
# background everywhere. The runtime outline radius uses the same value.
OUTLINE_RADIUS = 34.5

# Width used to detect a black line pixel as a contour between two parts.
CONTOUR_DETECT = 62.0
# A contour band thicker than this (distance to both neighbouring parts) is two
# merged strokes; each side keeps its half and no fill runs beneath it.
MERGED_BAND = 48.0

# ---------------------------------------------------------------------------
# Skeleton. pivot = rest world position (canvas px) of the bone origin, which
# is also the bone's rotation pivot. Rest rotation is 0 and rest scale is 1 for
# every bone, so bone-local axes are parallel to canvas axes at rest.
# ---------------------------------------------------------------------------
BONES = [
    # id, parent, pivot, safe rotation range (deg), note
    ("root", None, (624, 1066), (0, 0), "floor contact centre; whole-rig mirror axis"),
    ("hips", "root", (430, 1000), (-2, 2), "seat"),
    ("body", "hips", (440, 1030), (-2, 2), "torso sway pivot at the seat"),
    ("chest", "body", (500, 700), (-2, 2), ""),
    ("neck", "chest", (560, 540), (-3, 3), ""),
    ("head", "neck", (600, 530), (-10, 10), ""),
    ("ear_near", "head", (505, 366), (-12, 12), "root at the head junction: minimal slide over the cheek shading"),
    ("ear_far", "head", (905, 262), (-12, 12), "occluded by head at rest"),
    ("eye_left", "head", (605, 378), (0, 0), "blink = scale_y"),
    ("eye_right", "head", (900, 376), (0, 0), "blink = scale_y"),
    ("arm_near_upper", "chest", (446, 690), (-15, 15), "shoulder"),
    ("arm_near_lower", "arm_near_upper", (560, 704), (0, 0), "elbow (future mesh)"),
    ("paw_near", "arm_near_lower", (650, 736), (0, 0), "wrist (future mesh)"),
    ("arm_far_upper", "chest", (765, 690), (-15, 15), "occluded by laptop lid at rest"),
    ("arm_far_lower", "arm_far_upper", (835, 735), (0, 0), ""),
    ("paw_far", "arm_far_lower", (905, 772), (0, 0), ""),
    ("tail", "hips", (382, 1012), (-20, 20), "tail root under body"),
    ("leg_near_upper", "hips", (470, 976), (-15, 15), "hip"),
    ("leg_near_lower", "leg_near_upper", (800, 892), (0, 0), "knee (walk-ready)"),
    ("foot_near", "leg_near_lower", (792, 1040), (0, 0), "ankle (walk-ready)"),
    ("leg_far_upper", "hips", (640, 952), (-15, 15), "hip, behind near thigh"),
    ("leg_far_lower", "leg_far_upper", (990, 882), (0, 0), "knee (walk-ready)"),
    ("foot_far", "leg_far_lower", (1000, 1040), (0, 0), "ankle (walk-ready)"),
    ("laptop", "hips", (620, 868), (-2, 2), "base centre"),
    ("laptop_screen", "laptop", (706, 848), (-4, 4), "hinge"),
]

# ---------------------------------------------------------------------------
# Parts: one fill sprite + one line sprite each. z is draw order (low first).
# own: polygons (priority order = list order below) that claim *visible colour
#      pixels*. Line pixels are assigned automatically (see build script).
# ext: hidden-geometry extension shapes. They are clipped to the region
#      covered by higher-z parts at rest, so they never change the rest pose.
# ---------------------------------------------------------------------------
ORANGE = ["orange_light", "orange_dark"]
GREYS = ["grey_light", "grey_mid", "white"]

PARTS = [
    dict(id="eye_left", colors=["white"], bone="eye_left", z=90,
         own=[("ellipse", (605, 378, 52, 64))], ext=[]),
    dict(id="eye_right", colors=["white"], bone="eye_right", z=90,
         own=[("ellipse", (900, 376, 52, 64))], ext=[]),
    # the ear root under the head is not a real edge: no hidden contour there
    # (it would show through the head's feathered cut edge)
    dict(id="ear_near", colors=ORANGE, bone="ear_near", z=26,
         no_contour=[("poly", [(505, 236), (610, 236), (610, 410), (470, 410), (470, 372), (505, 372)])],
         own=[("poly", [(350, 240), (470, 228), (503, 258), (509, 298), (514, 372),
                        (476, 384), (432, 388), (396, 394), (350, 384)])],
         ext=[("poly", [(470, 236), (575, 250), (585, 372), (515, 372)])]),
    dict(id="head", colors=["orange_light", "white"], bone="head", z=80,
         own=[("poly", [(470, 205), (560, 110), (800, 70), (1010, 170), (1080, 380),
                        (1085, 566), (965, 586), (800, 600), (700, 603), (614, 591),
                        (570, 546), (546, 478), (518, 440), (478, 386), (514, 376),
                        (509, 298), (503, 258)])],
         ext=[]),
    dict(id="lid_inner", colors=["grey_light"], bone="laptop_screen", z=34,
         own=[("poly", [(596, 584), (748, 584), (704, 792), (556, 792), (556, 640)])],
         # the screen face has a straight, line-less left edge against the chest
         ext=[("poly", [(641, 588), (748, 588), (706, 806), (580, 806)])],
         no_contour=[("poly", [(560, 580), (643, 580), (583, 812), (520, 812)])]),
    dict(id="base", colors=["grey_mid"], bone="laptop", z=60,
         own=[("poly", [(488, 792), (640, 792), (640, 833), (748, 833), (748, 914),
                        (488, 914)])],
         ext=[("rrect", (500, 800, 660, 905, 40)), ("rrect", (600, 836, 752, 905, 30))]),
    dict(id="lid", colors=GREYS, bone="laptop_screen", z=50,
         # true top of the lid fill is the inner edge of its top stroke (y~604);
         # the chin stroke above it must not be backed by lid fill
         own=[("poly", [(690, 604), (1185, 604), (1185, 910), (872, 910), (744, 884),
                        (720, 833), (686, 833)])],
         ext=[("poly", [(700, 602), (1164, 602), (1110, 872), (680, 872), (690, 800)])]),
    dict(id="leg_near", colors=ORANGE, bone="leg_near_upper", z=70,
         own=[("poly", [(420, 895), (560, 902), (728, 902), (760, 866), (800, 846),
                        (842, 852), (872, 874), (846, 1080), (472, 1080), (445, 990)])],
         ext=[]),
    dict(id="leg_far", colors=ORANGE, bone="leg_far_upper", z=55,
         own=[("poly", [(872, 874), (905, 868), (960, 846), (1030, 842), (1100, 866),
                        (1150, 900), (1150, 1085), (846, 1085)])],
         ext=[("capsule", ((650, 965), (885, 950), 62))]),
    dict(id="arm_near", colors=ORANGE, bone="arm_near_upper", z=40,
         own=[("poly", [(452, 598), (470, 650), (600, 656), (662, 682), (704, 722),
                        (716, 806), (398, 806), (398, 682), (440, 662)])],
         ext=[("poly", [(492, 780), (696, 780), (712, 870), (505, 870)])],
         # the forearm underside merges into the body without a stroke (as at rest)
         no_contour=[("poly", [(380, 760), (575, 760), (575, 806), (700, 806), (700, 920),
                               (380, 920)])]),
    dict(id="tail", colors=ORANGE, bone="tail", z=10,
         own=[("poly", [(60, 870), (296, 870), (300, 900), (322, 950), (352, 1000),
                        (400, 1040), (450, 1052), (482, 1058), (482, 1120), (60, 1120)])],
         ext=[("poly", [(280, 880), (470, 930), (560, 1066), (280, 1066)])]),
    # `own` is a catch-all for colour ownership only; the body's real extent is
    # its visible colour plus `ext` (so it never backs strokes outside the torso)
    dict(id="body", colors=ORANGE, bone="body", z=20, own_is_extent=False,
         no_contour=[("poly", [(330, 300), (530, 300), (530, 440), (330, 440)])],
         own=[("poly", [(0, 0), (1254, 0), (1254, 1254), (0, 1254)])],
         ext=[# continues under the whole near-ear disc: the ear/back ownership
              # cut is not a real edge, so it must not get a hidden contour
              # chest continues behind the chin, visible when the head lifts
              ("poly", [(640, 530), (760, 540), (860, 536), (930, 526), (962, 552),
                        (962, 610), (640, 610)]),
              ("poly", [(352, 430), (360, 392), (382, 360), (420, 342), (470, 336),
                        (525, 342), (525, 430)]),
              ("poly", [(385, 430), (400, 398), (440, 380), (500, 352), (580, 332),
                        (700, 340), (760, 420), (770, 560), (700, 600), (360, 600)]),
              ("poly", [(330, 590), (770, 590), (740, 800), (660, 1066), (430, 1066),
                        (330, 1000), (290, 900), (320, 700)], "orange_dark")]),
]

# Part ids whose art is fully reconstructed (not present in the canonical
# image). They are generated procedurally by build_rig_v02.py.
RECONSTRUCTED = {
    "ear_far": dict(bone="ear_far", z=24, from_part="ear_near", mirror=True,
                    scale=0.9, centre=(912, 262)),
    "arm_far": dict(bone="arm_far_upper", z=30),
}

# Line pixels inside these polygons are forced to an owner (the fill beneath
# still comes from the nearest colour part unless `under` is given).
LINE_OVERRIDES = [
    # eye ovals: owned by the eye bones (blink squashes them), head fill beneath
    dict(owner="eye_left", poly=[(605 + 52 * __import__("math").cos(t / 16 * 6.2832),
                                  378 + 64 * __import__("math").sin(t / 16 * 6.2832)) for t in range(16)],
         under="head"),
    dict(owner="eye_right", poly=[(900 + 52 * __import__("math").cos(t / 16 * 6.2832),
                                   376 + 64 * __import__("math").sin(t / 16 * 6.2832)) for t in range(16)],
         under="head"),
    # whiskers, left: head line art drawn over chest / lid
    dict(owner="head", poly=[(460, 470), (560, 470), (640, 560), (610, 625), (520, 625),
                             (460, 560)], under=None),
    # cap of the arm's top contour: must travel with the arm
    dict(owner="arm_near", poly=[(405, 585), (475, 585), (475, 668), (405, 668)], under=None),
    # base's rounded bottom-left corner stays with the laptop base
    dict(owner="base", poly=[(492, 795), (560, 795), (560, 912), (492, 912)], under=None),
    # arm top contour where it passes the screen triangle's left corner
    dict(owner="arm_near", poly=[(560, 630), (700, 640), (705, 712), (560, 700)], under=None),
    # chin stroke merged with the lid's top stroke: the lid keeps its band,
    # the head carries the whole merged mass as its chin contour
    dict(owner="lid", also="head", poly=[(700, 500), (1000, 500), (1000, 612), (700, 612)],
         split_reach=36.0),
    # chin stroke at the screen triangle's apex
    dict(owner="head", poly=[(585, 568), (665, 568), (655, 606), (585, 606)], under=None),
    # end of the body's lower contour where it meets the floor line
    dict(owner="body", poly=[(380, 1015), (476, 1040), (476, 1072), (380, 1072)], under=None),
    # paw edge == lid edge below the Y junction: both need their own stroke
    dict(owner="lid", also="arm_near", poly=[(660, 700), (735, 700), (720, 806), (650, 806)],
         under="none", owner_reach=40.0),
    # thick outer-stroke mass in the notch between the near ear and the back:
    # regenerated by the runtime outline, never static line art
    dict(drop=True, poly=[(340, 318), (438, 318), (438, 420), (340, 420)]),
    # ear inner "C": ear line art, ear fill beneath
    dict(owner="ear_near", poly=[(440, 292), (482, 292), (506, 318), (520, 372),
                                 (474, 384), (440, 340)], under="ear_near"),
]

# Parts whose line-less cut edges over a lower part are feathered.
FEATHER_PARTS = ["head", "ear_near", "arm_near", "leg_near"]
FEATHER = 14.0

# Order used to resolve colour-pixel ownership (first match wins).
OWN_PRIORITY = ["eye_left", "eye_right", "ear_near", "head", "lid_inner", "base",
                "leg_near", "leg_far", "lid", "arm_near", "tail", "body"]
