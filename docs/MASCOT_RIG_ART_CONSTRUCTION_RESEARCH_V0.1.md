# Mascot Rig Art Construction Research v0.1

**Task:** MASCOT-RIG-RESEARCH-003 (research-only phase of MASCOT-RIG-WIN-002)
**Author:** implementation agent (independent research; not a restatement of Planner documents)
**Status:** research result for Planner review. Not an implementation authorization.
**Planner baseline read:** `77f0dac` (`docs: bound rig construction work to research only`)

References:

- Contracts: `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`, `docs/MASCOT_NEUTRAL_BIND_POSE_RESEARCH_SPEC_V0.1.md`, `docs/AGENT_ADDENDUM_RIG_ART_CONSTRUCTION_RESEARCH.md`.
- Canonical identity reference: `assets/mascot.png`.
- Non-canonical research reference: `assets/research/neutral-bind-pose/mascot-neutral-tpose-1x1-generated-v0.1.png` (see that directory's `README.md`).

Nothing in this document changes production code, art, rig data, clips, pivots or safe ranges. Numbers marked **measured** came from read-only analysis of the committed PNGs or the paused v0.2 rig. Numbers marked **proposed** are starting thresholds for later prototypes. They are not verified.

---

## 1. Executive technical conclusion

1. **Stop treating the flattened seated PNG as rig material.** Every class of defect found in the paused v0.2 pass comes from cutting a flattened illustration into pieces:
   - shared strokes cut in two;
   - straight-cut hidden fills;
   - shading bands clipped at part borders;
   - over-extended guesses at hidden shapes.

   Live2D, Spine and Toon Boom all start from layered art in which every part is drawn complete, including the parts hidden in the display pose (sources S1, S2, S7). Mascot should do the same.

2. **Adopt a purpose-built articulated art master, but do not make a front-facing T-pose its bind pose.** Recommended: a **complete-part master assembled in the canonical right-facing 3/4 seated pose**. That pose is the bind/rest pose, so the most-visible frame needs zero deformation. Parts are authored complete and unoccluded, and can also be viewed as an exploded parts sheet. Large future posture families (standing, walking, dangling while dragged) should come from **attachment substitution** with their own bind meshes, not from bending one T-pose master through 70–100°. See §4–§5. This disagrees with the T-pose framing of the generated reference and is listed as decision D1 in §19.

3. **Separate the concerns and give each one its own mechanism:**

   | Concern | Mechanism |
   |---|---|
   | External contour | runtime, from the final composed/deformed fill silhouette (already implemented) |
   | Contact line between independent objects (chin/lid, paw/lid, foot/laptop) | owned by the **front** object, drawn above the back object's colour and below the front object's colour, **clipped to the back object's posed coverage** |
   | Continuous anatomy (hip/thigh, shoulder/upper arm, tail root, torso) | local weighted meshes with explicit weights |
   | Intrinsic lines (eyes, nose, mouth, whiskers, ear C-crease, laptop logo) | authored; move/deform with the owning surface via the same UVs/mesh |
   | Directional two-tone shading | runtime, screen-fixed "top light" rule (see item 4) |
   | Albedo, markings, intrinsic lines | authored |

4. **Shading: the canonical style suits runtime generation.**
   - **Measured:** in the canonical PNG, limbs use a dark base with light inset highlights on the upper surfaces (arm, thigh, foot).
   - Their lower edges are **flat and horizontal in screen space**.
   - A rule of the form `highlight = inset(part fill) ∩ screen-space half-plane` keeps those edges horizontal when a limb rotates. A baked highlight would tilt with the limb and, on larger rotations, end up on the wrong side.
   - This needs a small prototype (C, optional) before adoption.

5. **Renderer.** The animation model is already sufficient: `Attachment::Mesh` plus 2D linear-blend skinning in `skin_mesh`. The data path to the renderer is missing:
   - `Rig::draw_list` drops meshes;
   - `DrawItem` can only carry an affine;
   - the renderer never uploads mesh textures.

   The narrowest robust native path is a **D3D11 textured-triangle pass into the renderer's existing D2D target bitmaps** (`ID2D1Bitmap1::GetSurface` gives a render target view), with CPU skinning and one dynamic vertex buffer. Direct2D `FillMesh` is not usable for textured skinning (S10). See §11.

6. **Most of the v0.2 tooling survives.** Runtime outline, clip 0.3 authoring, `mascotctl`, the frame artifact detector and the stress/sweep workflow are KEEP/ADAPT. The seated decomposition, junction morphology, pivot heuristics and provisional safe ranges are RETIRE or RESEARCH ONLY. See §15.

---

## 2. Sources and extracted principles

Each row gives the principle, whether it applies to Mascot, what does not transfer, and the resulting recommendation.

| # | Source | Principle | Applies to Mascot | Does not transfer | Recommendation |
|---|---|---|---|---|---|
| S1 | Live2D tutorial "Illustration processing", https://docs.live2d.com/en/cubism-editor-tutorials/psd/ | "Do not forget to add the parts that were hidden in the source image"; "The neck should be drawn larger so that it is not visibly cut off when the face is moved… up to the mouth area"; arms split into upper arm, forearm and hand; make hidden additions generous from the beginning | Directly: complete parts, generous hidden continuation, neck under head extending to about mouth level, arm split | Live2D **merges line art and fill into one layer per part** ("line art and fill must be combined into a single layer"). Mascot must keep fill and line separate, because the fill silhouette drives the runtime outline and contact lines | Complete-part master with separate fill / intrinsic-line / marking layers per part |
| S2 | Live2D manual "About Material Separation", https://docs.live2d.com/en/cubism-editor-manual/divide-the-material/ | Keep a non-destructive separation master (masks, groups) separate from the flattened import file; draw first the parts that move most or become visible when others move | Master-vs-derived split maps onto a layered art master vs. generated `rig-v0.2/parts/*.png` | PSD/Photoshop tooling requirement | Keep a layered art master as authoring source; runtime part PNGs are derived build output |
| S3 | Live2D manual "Rotation deformer", https://docs.live2d.com/en/cubism-editor-manual/making-and-rotation-of-rotationdeformer/ | Rotation is authored as a pivot + parent–child hierarchy of deformers; the pivot is placed deliberately | Pivots are authored semantics (Planner §2.2 agrees) | Parameter/keyform blending (Live2D interpolates authored mesh shapes per parameter) is a different model from bone skinning | Keep authored pivots; do not adopt keyform blending |
| S4 | Live2D manual "Glue", https://docs.live2d.com/en/cubism-editor-manual/glue/ ; "Skinning", https://docs.live2d.com/en/cubism-editor-manual/skinning/ | Two ArtMeshes can be bound at overlapping vertices with per-vertex glue weights; Live2D "skinning" splits a mesh between rotation deformers and glues the halves | A continuous surface can be one mesh with blended weights **or** two meshes whose seam vertices share weights. The seam then cannot open | Live2D skinning is documented for long thin meshes (hair, strings); a "full skinning function… will be supported in the future" | For Mascot, one mesh per continuous region with explicit weights is simpler than glue; keep glue as the fallback if two textures must meet seamlessly |
| S5 | Spine "Slots", https://esotericsoftware.com/spine-slots ; "Basic concepts", https://esotericsoftware.com/spine-basic-concepts | Draw order is a list of slots; slots decouple bones from draw order ("attachments on the same bone … drawn above and below an attachment on a different bone"); attachment swapping within a slot | Mascot already has global `z` independent of bones. Substitution (swapping attachments) is the standard answer for large pose changes | Spine skins and full attachment libraries are more than current needs | KEEP explicit z; add attachment substitution only when a second posture family exists |
| S6 | Spine "Meshes", https://esotericsoftware.com/spine-meshes ; "Weights", https://esotericsoftware.com/spine-weights | Mesh = textured polygon with a hull. Vertices go around important features first, then fill in. The hull can exclude transparent image regions. Weights are linear blend skinning; each vertex has a weight per bone. Auto weights are a starting point and are then edited. Triangles of a self-overlapping mesh are ordered by their dominant bone | Directly: feature-placed vertices, explicit editable weights, deterministic auto-weights as a starting point. **Self-overlap needs triangle ordering**, so Mascot meshes should avoid self-overlap: the thigh mesh lies over a separate body slot rather than folding onto itself | Spine's topology-aware auto-weight algorithm is proprietary. Free-form deform keys are unnecessary now | Mesh rules in §8 and addendum R-M1…R-M9 |
| S7 | Toon Boom Harmony "About character breakdown", https://docs.toonboom.com/help/harmony-25/essentials/rigging/about-character-breakdown.html | "Animated parts … must be properly overlapping and complete. A common mistake is to trace the part just as it appears on the model"; hands kept as one layer with a bank of drawings (substitution) | Direct confirmation of the v0.2 failure: rest-pose tracing ≠ articulated parts. Substitution banks for hands/paws | Harmony's vector node compositing | Complete overlapping parts; paw/hand substitution for future typing/waving poses |
| S8 | Toon Boom Harmony "About Auto-Patch articulations", https://docs.toonboom.com/help/harmony-24/premium/rigging/about-auto-patch-articulation.html ; Toon Boom help centre "How does the Auto-Patch node work" | Draw both pieces with **complete lines overlapping in semicircles**. A patch is the lower piece's colour art clipped inside its own line, placed in front of the joint so the joint line is hidden without cutting any outline. This needs separate line and colour layers | Mascot's composition already works this way: runtime outline under colour. The patch idea is the same as the Planner's joint underpaint (a colour-only safety layer). It also shows that line suppression is a **composition** decision, not an art-cutting decision | Harmony's semicircle construction assumes a hinge at a rounded end; anatomical flows (rump→thigh) still need deformation | Contact lines and joint safety layers are composition rules with explicit per-pair data (§9) |
| S9 | Blender manual "Freestyle line set", https://docs.blender.org/manual/en/latest/render/freestyle/view_layer/line_set.html | Line classes are separate selectable edge types: **External contour** ("lines around all objects, separating them from the scene background, but not each other"), **Contour** ("separating it from other objects behind it, or the scene background"), **Material boundary** ("where two materials meet on the same object"), **Edge marks** (explicitly marked edges, rendered when the Edge Mark type is selected) | Maps one-to-one onto Mascot's classes: External contour = class A; Contour restricted to chosen object pairs = class B; Edge marks = local edge masks; intrinsic lines ≈ authored marks | 3D view-dependent edges (silhouette/crease/suggestive) have no 2D equivalent | Class B is "contour against specific objects behind", selected by pair policy and optional edge mask |
| S10 | Microsoft Learn `ID2D1RenderTarget::FillMesh`, https://learn.microsoft.com/en-us/windows/win32/api/d2d1/nf-d2d1-id2d1rendertarget-fillmesh | FillMesh requires aliased antialias mode and takes one brush with one transform. It cannot map per-triangle UVs | Rules FillMesh out for textured skinning | n/a | Do not use FillMesh |
| S11 | Microsoft Learn "Direct2D and Direct3D interoperability overview", https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-direct3d-interoperation-overview ; `ID2D1Bitmap1::GetSurface`, https://learn.microsoft.com/en-us/windows/win32/api/d2d1_1/nf-d2d1_1-id2d1bitmap1-getsurface | D2D and D3D11 interoperate through DXGI surfaces on the same device. `GetSurface` returns the bitmap's underlying surface for use with Direct3D on that device. DXGI surface render targets do not synchronize, so the app must order D2D and D3D work (flush/EndDraw) | The renderer already owns a D3D11 device and D2D device context (`Renderer { d3d, device, ctx, … }`), and its scratch bitmaps are D2D target bitmaps | n/a | D3D11 pass writing into the D2D scratch bitmaps, with D2D `EndDraw` before each D3D mesh draw and `BeginDraw` after |
| S12 | Microsoft Learn `ID2D1DrawInfo::SetVertexProcessing` / custom effects, https://learn.microsoft.com/en-us/windows/win32/api/d2d1effectauthor/nf-d2d1effectauthor-id2d1drawinfo-setvertexprocessing | D2D custom effects can supply a vertex buffer and vertex shader | A D2D-native textured-triangle path exists | Requires implementing COM effect interfaces and registration from Rust, which is more code and risk than a direct D3D11 draw | Rejected as the first path; documented alternative |
| S13 | Spine "Physics constraints", https://esotericsoftware.com/spine-physics-constraints | Secondary motion is a **constraint on bone properties** (inertia, strength, damping, mass, gravity, mix), applied after animation. Deterministic mode replays from frame 0 | Physics/ragdoll can later sit on top of the same bones and meshes. Meshes only need correct bone transforms | Spine physics is per-bone secondary motion, not full rigid-body contact | Architecture requirement: meshes and art must not assume bone transforms stay inside clip ranges (§13) |

**Disagreements and tradeoffs between approaches**

- **Line ownership.** Live2D merges lines into each part's texture. Harmony separates line and colour and patches joints. Blender classifies edges at render time. Mascot needs Harmony/Blender-style separation, because its external outline is already runtime-derived, and a baked per-part outline is exactly what caused v0.1's doubled contours.
- **Continuity.** Live2D prefers keyform deformation (authored shapes per parameter). Spine prefers bone skinning. Mascot already has bone skinning in its model and wants cheap agent-authored clips, so skinning with a few explicit weights is the fit. Keyforms would add a second animation model.
- **Large pose changes.** Mesh deformation or drawing substitution? All three tools deform for small and medium ranges and substitute for large changes of view or pose. Mascot should do the same.

---

## 3. Failure analysis of the current seated source

**Measured on the paused v0.2 rig and `assets/mascot.png`.**

| Failure | Mechanism | Why repair of the flattened source cannot fix it generally |
|---|---|---|
| Shared strokes cut in two (chin/lid, paw/lid, foot/base, ear rim/back) | One black stroke separates two objects in the flat image; decomposition gives a fragment to each | The owner and the hidden continuation of the stroke do not exist in the source |
| Cut-paper hidden fills (shoulder, hip, tail root, far foot, below ear) | Hidden regions filled by extension/morphology with straight cut boundaries and truncated shading bands | The shading of the hidden area is unknown; the flat image fixes only the visible shading |
| Over-extended hidden shapes (laptop screen block) | Global extension distances | Correct extent depends on each joint's motion envelope (Planner §4.1) |
| Rigid "limb" pieces | `arm_near.fill` spans upper arm, forearm and paw on one bone (`arm_near_upper`); `arm_near_lower` and `paw_near` carry no attachment. Legs likewise | Elbow/knee/paw motion is impossible without redrawing parts |
| Detector-clean but visually broken | `arm_near` ±15° passes the automatic detector yet shows raw cuts | Construction faults are semantic; heuristics see only pixel statistics |
| Narrow, asymmetric clean ranges | e.g. `leg_near_upper` −4..0°, `ear_near` −2..+5° against desired ±15/±12 | Rigid pieces cut from a continuous surface cannot rotate without exposing the cut |
| Laptop is a body bone | `laptop` is a child of `hips`, and body art under the laptop is not complete | Prop cannot be moved, removed or held; body under it would be invented at repair time |

**Conclusion:** repairing the seated decomposition means inventing, part by part, the hidden anatomy that a layered master would contain anyway. It also locks the invented regions to one pose. Continuing it is sunk-cost repair (Universal Gate 17).

---

## 4. Neutral-master decision

**Options considered**

| Option | Description | Verdict |
|---|---|---|
| M0 | Continue decomposing the flattened seated PNG | Reject (see §3) |
| M1 | Neutral T-pose master; canonical seated pose reconstructed by posing | Reject as bind pose (see §5) |
| M2 | **Complete-part layered master assembled in the canonical seated pose** (bind = canonical rest) | **Recommend** |
| M3 | Hybrid: M2 plus later posture families (standing, dangling) as substitution attachment sets with their own bind meshes | Recommend as the growth path of M2 |

**Why M2**

- **Rest identity by construction.** The most-visible frame is the bind pose, and skinning at bind is identity. Rest fidelity then depends only on the quality of the redrawn art, not on deformation error.
- **Deformation stays small where the product lives.** Idle clips (blink, look, tilt, ear twitch, tail flick, posture adjust, stretch) are small offsets from the seated pose, which is where linear blend skinning is accurate.
- **Complete, unoccluded art is still required.** Torso under the arm, lap and belly under the laptop, the far arm, both paws, full neck under the head, tail root, ear roots, and the complete laptop as a prop. These are drawn as **separate layers** in the seated assembly and can be viewed as an exploded parts sheet.
- **Matches the practice of all three tools** (S1, S7): art is prepared in the display pose, with hidden parts added.

**What the neutral T-pose reference is good for (research only)**

- Hypotheses about the shape of anatomy the canonical image hides:
  - torso front;
  - a full far arm;
  - both legs with feet;
  - the tail root's attachment to the rump;
  - shoulder and hip surfaces.
- Skeleton topology sanity: a 2-bone arm, 2-bone leg with foot, and a 3–4 bone tail are all readable.

**Where the neutral reference loses identity or detail relative to the canonical (measured / observed)**

| Aspect | Canonical `assets/mascot.png` | Neutral generated reference |
|---|---|---|
| Line weight | stroke median ≈29 px (p25 25, p75 33; ridge method at 1254 px; the rig's own measurement reports median ≈35 px) | median ≈15 px, about half the weight, so a different line language |
| Shading grammar | dark orange base (`#e48424` ≈40% of coloured pixels) with light inset highlights (`#fc9c3c` ≈39%) whose lower edges are flat and screen-horizontal | light base (`#fc9c24` ≈53%) with dark shadow bands along left/lower edges; the inverse grammar |
| Body markings | no visible cream belly; body front is hidden by the laptop | large cream belly/chest (`#fce4cc` ≈17%), an invented identity feature |
| View consistency | head and body both right-facing 3/4 | 3/4 head on a near-frontal torso, an inconsistent view |
| Proportions | compact bean body; head ≈ half the figure; short stubby limbs | taller, slimmer torso; long T-pose arms; longer legs |
| Extremities | paws and feet are simple rounded shapes without toe lines | toe/finger lines on hands and feet (added detail) |
| Eyes / whiskers | vertically oval eyes; right whiskers cross the head contour | rounder eyes; right whiskers drawn as short ticks inside the contour |
| Silhouette | bbox 1076×975 px, silhouette fraction 0.452 | bbox 1092×1158 px, silhouette fraction 0.424 |

These differences make the neutral image unsuitable as tracing material (the neutral-bind-pose README forbids that anyway). It is also unsuitable as proportion authority. It stays useful as an anatomy sketch.

---

## 5. Bind-pose recommendation

Criteria are the ones in the neutral research spec §5. Ratings are relative: ++ best, −− worst.

| Criterion | Strict T-pose (front) | Relaxed A-pose (front) | Right-facing 3/4, limbs slightly separated | **Canonical seated 3/4 (M2 bind)** |
|---|---|---|---|---|
| Identity preservation | −− (view change) | − | + | **++** |
| Near/far limb readability | + (left/right) | + | ++ | + (far limbs mostly hidden; need layers) |
| Shoulder/hip topology for skinning | + (clean) | ++ | + | + |
| Neck/head articulation | − (head view differs from body view) | − | ++ | ++ |
| Tail-root topology | − (tail behind leg) | − | + | + |
| Paw/foot readability | + | + | + | + (paw on lid, feet forward) |
| Deformation to reach the main (seated/laptop) pose | −− (70–100° shoulder, about 90° hip) | − (about 45°) | − (hip flexion) | **++ (zero)** |
| Future walking/standing | + | + | + | − → substitution family |
| Laptop interaction | − | − | − | ++ |
| Active ragdoll / dragging | + (limbs unfold) | + | + | − (dangling needs large ranges; substitution or large-range meshes) |
| Pose-specific art needed | low for standing, high to reach seated fidelity | medium | medium | low for seated; one extra family for standing |

**Recommendation:** bind to the canonical seated 3/4 pose (M2). Add a standing/"held" family as substitution attachments later (M3), bound to the **same skeleton**. It must have its own bind meshes, drawn in a relaxed 3/4 standing pose with arms separated from the torso (the third column). A front-facing T-pose is not recommended for Mascot at any stage, because its view is inconsistent with the character's identity.

---

## 6. Complete-art / material separation contract

For a future production master, each item below is required (R), recommended (Rec) or unnecessary (U).

| Item | Status | Note |
|---|---|---|
| Complete torso under arms, lap and belly under laptop | R | Belly colour/marking is an identity decision (D3) |
| Complete shoulder and hip surfaces | R | Include the rump contour continuation that flows into the thigh |
| Complete near/far upper and lower limbs, split upper / lower / paw | R | Follows S1's arm split; enables elbow/knee/paw later |
| Complete paws/feet | R | Paws as substitution candidates (S7) |
| Complete neck under head, extending to about mouth level | R | S1 |
| Complete tail root continuing into the rump | R | Tail mesh root overlaps the rump |
| Complete ear roots inside the head silhouette | R | Concealed-root rigid rotation |
| Complete head surface beneath eyes, highlights and whiskers | R | The v0.2 blink regression came from missing fill here |
| Laptop as an independent prop: lid outer, lid inner/screen, base, hinge | R | Separate prop rig (§12) |
| Intrinsic markings (muzzle white, nose, eye whites/pupils) separate from directional shading | R | Needed for runtime shading (§10) |
| Separate layers per part: `fill` (albedo), `line` (intrinsic lines only), optional `mark` | R | Outer contour **not** drawn per part |
| Semantic line-ownership metadata (per stroke: class A/B/C, owner, partner) | R | Machine-readable (§9) |
| Mesh-ready source: high-resolution raster (≥2× runtime) or vector, transparent padding ≥8 px at 1× around every part | Rec | Padding allows a hull outside alpha for anti-aliased mesh edges |
| Per-part hand-painted directional shading | U if runtime shading adopted; otherwise R | Decision D2 |

---

## 7. Body-region deformation map

These are the recommended starting classes for the M2 master. **Desired** ranges are the committed product targets in `rig.json` at HEAD (the paused local copy has narrower provisional values that are diagnostics only).

| Region / joint | Bones | Desired | Recommended class | Rationale |
|---|---|---|---|---|
| Torso (posture, breathing) | hips, body, chest | ±2° | **weighted mesh** (torso, 3 bones) | Currently a rigid body sprite with rigid chest/neck rotation only moving children. A torso mesh is also the parent surface for shoulder/hip meshes |
| Neck / head | neck, head | neck ±3°, head ±10° | head **rigid-socket-small-range** over a complete neck continuation (S1); torso mesh carries the neck region | The head is a separate object silhouette; chin/lid contact is class B. A neck mesh is only needed if neck bends grow past about ±5° |
| Ears | ear_near, ear_far | ±12° | **rigid-socket-small-range**, concealed rounded root inside the head silhouette | Small ranges; the ear C-crease is intrinsic (owned by the ear) |
| Eyes | eye_left, eye_right | scale only | rigid (scale), intrinsic lines | Head fill continuous beneath |
| Shoulder / upper arm (near) | chest → arm_near_upper | ±15° | **weighted mesh** (shoulder cap weighted to chest, upper arm to arm bone) | Measured rigid failure; continuous tube in this style |
| Elbow / forearm (near) | arm_near_upper → arm_near_lower | future | **weighted mesh chain** (same mesh as the upper arm, 2 bones) | The canonical "C" arm is one continuous tube; a hinge would show a cut |
| Paw (near) | arm_near_lower → paw_near | future | **rigid-hinge** + substitution bank; contact line with lid (class B) | Paw is a distinct rounded object |
| Far arm | chest → arm_far_* | ±15° | rigid concealed socket (mostly occluded by laptop and body) | Promote to mesh only if motion exposes it |
| Hip / thigh (near) | hips → leg_near_upper | ±15° | **weighted mesh** (rump weighted to hips, thigh to leg) | Measured rigid clean range −4..0°; the rump contour must flow into the thigh |
| Knee / foot (near) | leg_near_lower, foot_near | future | knee in the same mesh as the thigh (2 bones); foot rigid-hinge; foot/base contact line | Seated leg is a horizontal tube plus foot |
| Far leg | hips → leg_far_* | ±15° | rigid concealed socket first; mesh if the sweep shows an exposed seam | Largely hidden behind the base and near leg |
| Tail | hips → tail (future tail_1..3) | ±20° | **multi-bone chain + weighted mesh**, root weighted to hips | Long flexible appendage with a continuous root |
| Laptop lid | laptop → laptop_screen | ±4° | rigid-hinge (lid over base, concealed hinge) | Mechanical object |
| Laptop prop mount | prop root | 0 now | separate prop rig (§12) | Not body skin |

**Joint underpaint** (Planner §17.3) is kept only as a concealed safety layer under rigid sockets (ears, head, far limbs). Weighted meshes do not need it.

### 7.1 Proposed art-readiness checks (per joint, before clips)

1. **Construction:** no dark component in the joint ROI without a class A/B/C owner (line-ownership inventory), and no raw cut edge at any sampled angle.
2. **Static sweep:** at least 9 samples across the desired range plus 1.25× diagnostic extremes, at 1× and 4×–8× crops, with the existing detector signals **and** mandatory visual review. Detector-clean alone is insufficient: `arm_near` ±15° was detector-clean but visually broken.
3. **Rigid sockets:**
   - the concealed root stays inside the parent silhouette at every sampled angle;
   - underpaint is never visible as a flat patch;
   - no black perimeter fragment is exposed (Planner §10.3).
4. **Meshes:** the §8.4 thresholds, plus rest identity.
5. **Contact pairs:**
   - at every sample the class B line is continuous along the contact;
   - no orphan fragments;
   - no line over the background.
6. **Pivot:** the contact point drifts ≤ 0.25× stroke width over the sweep for contact-constrained pivots (the chin on the lid).
7. **Output:** a verified range per joint (possibly asymmetric), recorded next to the desired range, plus a joint sheet as evidence.

---

## 8. Mesh topology and weights recommendation

### 8.1 Suitability of the existing model (read-only inspection)

- `MeshAttachment { image, vertices (canvas px, bind), uvs, triangles, weights }` and `skin_mesh` (`crates/mascot-animation/src/attachment.rs:19-28, 104-116`) are sufficient for 2D linear blend skinning. They have a bind-identity test.
- Gaps found:
  1. `Rig::draw_list` returns `None` for meshes (`crates/mascot-animation/src/lib.rs:107-109`), and `DrawItem` carries only an affine (`lib.rs:44-52`).
  2. The Windows renderer uploads no mesh bitmaps (`crates/mascot-render-win32/src/renderer.rs:347`). Its fill bounding box assumes bitmap corners (`renderer.rs:481-493`).
  3. Validation (`crates/mascot-animation/src/rig.rs:231-240`) checks counts and bone names only. It does not check:
     - that each vertex has at most 4 influences (which the doc comment states);
     - that weights are non-negative;
     - triangle index bounds;
     - degenerate/zero-area triangles;
     - consistent winding;
     - the UV unit convention.
  4. `Slot::from_def` **silently renormalizes** weights (`attachment.rs:58-75`). The construction spec requires authored weights that sum to 1, so non-normalized data should be rejected, not silently rewritten. Hard cut, no fallback.
  5. One `image` per mesh. A meshed part with both `fill` and intrinsic `line` layers would need two attachments that duplicate the same geometry, which Universal Gates 7/9 prohibit. Proposed shape: one mesh geometry referenced by the part's fill and line images (both sampled with the same UVs).
  6. The pivot-shift tool rejects meshes (`rig.rs:265-300`). That is acceptable if mesh binds are explicitly canvas-space. Document it rather than work around it.
  7. `mascot-qa` ownership and detector ROIs assume sprite transforms and need a mesh path.

### 8.2 Topology rules (proposed)

- Mesh only the continuous region: the hip mesh covers rump + thigh (+ knee later), not the whole body. The torso mesh is separate and lies beneath it.
- The hull lies in transparent padding 2–4 px outside the alpha edge at 1×. That gives anti-aliased edges from texture alpha and no edge anti-aliasing requirement on triangles.
- Density:
  - in the transition band, vertex spacing ≈ 1–1.5× the stroke width (≈30–45 px at 1254 px canvas);
  - outside it, one row of support vertices and coarse triangles (≥100 px);
  - transition band cross-sections are edge loops perpendicular to the bone.
- No self-overlap within one mesh. Overlap between regions is expressed by separate slots and z-order (S6 triangle ordering is avoided).
- Bind triangles need a minimum interior angle ≥15°, generated by constrained Delaunay from the hull plus feature loops.

### 8.3 Weights (proposed)

- At most 2 influences per vertex for single joints; at most 3 at triple junctions (e.g. hip vertices near the tail root: hips / leg / tail). Prune influences below 0.02 and renormalize **in the authoring tool**, never at load time.
- Initial weights are deterministic: a smoothstep across a band centred on the joint pivot, measured along the child bone axis.
- Joint-specific band widths as a fraction of limb width at the joint:

  | Joint | Band width |
  |---|---|
  | hip | 0.4–0.5 |
  | shoulder | 0.3–0.4 |
  | tail root | 0.5 |
  | torso segments | 0.6 |

- The generator's output (vertices, triangles, UVs, weights) is committed as the **single canonical mesh data**, is hand-adjustable, and is inspected via `mascotctl`, not by reading raw JSON. The generator's input parameters are not a second canonical source.
- Linear blend skinning volume loss is negligible in 2D at ≤ ±20° with bands this wide. Dual-quaternion or rotational blending is not needed. Revisit if a range above about ±45° becomes necessary.

### 8.4 Mesh QA thresholds (proposed; joint-specific; verify in Prototype B)

For each triangle, take the singular values σ1 ≥ σ2 of its 2×2 bind→posed deformation:

| Check | Fill-only triangles | Triangles covering intrinsic or contour line art |
|---|---|---|
| Inversion | det > 0 everywhere in the safe range | same |
| Area ratio σ1·σ2 | 0.75–1.33 | 0.85–1.18 |
| Anisotropy σ1/σ2 | ≤ 1.35 | ≤ 1.15 (a 29 px stroke then changes by ≤ about 4 px) |
| Posed minimum area | ≥ 25% of bind | ≥ 50% of bind |

Also required:

- **Rest identity:** bind render vs. the accepted sprite/master render inside the mesh region, with no pixel differing by more than 2/255 per channel away from alpha edges.
- **Coverage:** no texture holes or background exposure. Posed coverage must equal the union of triangle coverage, with alpha > 0.5 over the whole bind-opaque region mapped forward.

---

## 9. Line ownership / contact-line recommendation

### 9.1 Classes

These are the Planner's A/B/C classes, specialized with Blender edge types (S9):

| Class | What it is | Mechanism |
|---|---|---|
| A | External contour (Freestyle "External contour") | Current runtime: `dilate(fill silhouette, r)` drawn under colour (`renderer.rs:469-553`). KEEP. It follows deformation automatically once meshes contribute to the fill pass |
| B | Contact line (Freestyle "Contour", restricted to listed object pairs) | A stroke owned by the **front** object F, visible only where it overlaps the **back** object B |
| C | Intrinsic line | Authored `line` layer of one part; sampled through the same sprite transform or mesh UVs as its fill, so it deforms with the surface |

### 9.2 Class B composition rule (proposed)

    colour(B) … colour(objects between B and F in z) …
        → contact_line(F, B)   clipped to posed coverage(B) and to edge_mask(F)
        → colour(F)

- **Front selection:** the higher-z member of the pair at the current pose. If the order can change, the policy is keyed by order.
- **Placement:** drawn directly under F's colour, so F's anti-aliased edge covers the inner half of the line. This is the same principle as the external outline and Harmony's line-under-colour patching (S8).
- **Clip to B's coverage:** the line never hangs over the background. There, class A takes over, which prevents doubled lines where F's edge meets the silhouette. Anything z-ordered between B and F occludes the line naturally.
- **Edge mask:** an alpha texture in F's local image space that moves or deforms with F. The mask is both **where** a line exists and **how thick** it is:
  - the generated ring extends outward from F's edge by up to `w`;
  - a mask band whose extent shrinks from `w` to 0 produces a real geometric **taper** at stroke ends, not just an opacity fade.
- **No line where the pair has no policy.** Shoulder, neck, hip and tail-root boundaries never get class B lines. Continuity there is a mesh concern (Planner §17.8).

### 9.3 Two candidate line sources for class B

Prototype A decides between them.

- **V1, owner stroke:** the source contact stroke is re-authored as an F-owned stroke texture that continues past the visible rest extent (a "hidden line continuation", analogous to hidden fill), with the clip rule above. It gives the best rest fidelity and is trivially cheap (one masked bitmap draw per pair). It does not adapt if F's contour deforms.
- **V2, generated ring:** `dilate(alpha_F, w) ∩ edge_mask_F ∩ coverage_B`, which reuses the existing ring-stamp dilation on F's fill only. It adapts to deformed or meshed contours and has constant width, but rest fidelity depends on how well a uniform-width ring matches hand-drawn strokes.

**Rejected: part-ID buffer + edge detection**

- An ID buffer is aliased by construction, which conflicts with anti-aliased strokes.
- It stores only the top-most part, so it cannot provide B's coverage under F's ring.
- It needs readback or a custom shader, whereas pairwise masks reuse existing D2D compositing (opacity masks / layers).

**Expected cost:** one additional F-silhouette dilation (V2) or one masked draw (V1) per active pair, only on frames that render. The event-driven rendering policy is unaffected.

---

## 10. Runtime shading recommendation

### 10.1 Observations (measured / observed)

- Canonical limbs use a two-tone scheme: dark orange base and light orange highlight shapes inset from the contour on the upper side of the arm, thigh and foot.
- The highlights' **lower boundaries are straight, horizontal lines in screen space** with slightly rounded corners.
- The head is uniformly light. The large torso side is dark base without a highlight.

### 10.2 Consequence

If highlights stay baked into rotating limb textures:

- at ±15° the flat highlight edge visibly tilts with the limb;
- at larger ranges (future dragging, standing family) the highlight migrates to the wrong side;
- on meshes the bands stretch with the surface.

### 10.3 Proposed stylized model

This is not physically based. For each **shading group** (a part or mesh region with a highlight):

    highlight = inset(fill_group, d_inset)
              ∩ screen_half_plane(y <= anchor_y(group, pose) + h_group)
              (optionally rounded by a small blur+threshold)
    colour = albedo_dark  where fill;   albedo_light where highlight

- `anchor_y` follows a bone point, so the highlight stays on the limb. Its lower edge stays screen-horizontal (a fixed "top light").
- `inset` is erosion of the group fill. That is the dual of the existing ring dilation: MIN instead of MAX blending, or dilation of the inverse.
- The half-plane is an axis-aligned screen clip, which is cheap in D2D.
- Deterministic.
- No normal maps or per-vertex normals are needed at Mascot's style level. If a group needs a non-horizontal terminator, a per-group angle parameter suffices.

### 10.4 What stays baked vs. runtime

| Baked (authored) | Runtime |
|---|---|
| Albedo base (dark orange), light cream/white markings (muzzle), nose/eyes, intrinsic lines, laptop materials and logo | Two-tone limb highlights (shading groups), external outline, class B contact lines, drop/contact shadow (already runtime) |
| Contact AO only where it is a fixed design feature of one part (e.g. inner ear) | Contact darkening between independent objects, if ever wanted (same pairwise-clip rule as class B) |

**Caveat / decision D2:** this changes how the canonical look is produced. Hand-shaped highlight silhouettes will be approximated. Adoption requires Prototype C to show acceptable rest fidelity. Until then, small-range rigid parts may keep baked highlights.

---

## 11. Renderer implications

### 11.1 Options evaluated

| Path | Description | Pros | Cons | Verdict |
|---|---|---|---|---|
| R1 | **D3D11 textured triangles into the D2D scratch target bitmaps** | Watertight rasterization (no internal seams); one `DrawIndexed` per mesh slot per pass; premultiplied blend matches D2D; device already owned | Needs HLSL vertex/pixel shaders (precompiled bytecode) and explicit D2D/D3D ordering; about 300–500 lines | **Recommended** |
| R2 | D2D per-triangle `FillGeometry` with a bitmap brush transformed per triangle | No shaders | Anti-aliased mode shows seams between adjacent triangles. Aliased mode depends on watertight rasterization, which D2D does not document. One geometry + brush change per triangle | Fallback only |
| R3 | D2D custom effect with vertex buffer (S12) | Stays inside the D2D effect graph | COM effect implementation and registration from Rust; highest complexity | Reject for now |
| R4 | CPU reference rasterizer → upload bitmap per frame | Deterministic, trivially testable | CPU cost and upload per frame; a second production raster path | **Use as QA oracle and in Prototype B only**, not in production |

### 11.2 R1 shape

- **Ownership:** mesh drawing lives in `mascot-render-win32`. Skinning stays in `mascot-animation` (`skin_mesh`). The renderer receives already-skinned positions.
- **Minimum data path:** `draw_list` emits a draw item that is either a sprite affine or a mesh reference with skinned canvas-space positions. The renderer applies the view affine on the CPU or in a constant buffer.
- **Minimum GPU resources:**
  - one dynamic vertex buffer (position + UV, `Map(WRITE_DISCARD)` per frame);
  - one static index buffer per mesh;
  - one SRV texture per mesh image;
  - an RTV on the current D2D scratch target via `GetSurface`;
  - one premultiplied blend state (ONE, INV_SRC_ALPHA);
  - a linear-clamp sampler;
  - one VS and one PS.
- **Batching:** meshes are interleaved with sprites by z. Each mesh slot ends the current D2D draw (`EndDraw`), issues the D3D draw, then resumes D2D (`BeginDraw`). Expected mesh slots: at most 6 (torso, 2 arms, near leg, tail, maybe far leg), times 2 passes (colour, fill). If this flush cost shows up in profiling, the follow-up is to draw sprites as 4-vertex meshes in the same D3D pass (Spine represents regions the same way, S6). That would make one geometry path.
- **Alpha / composition:** texture alpha provides edge anti-aliasing because the hull lies in transparent padding. No MSAA is needed.
- **Outline compatibility:** meshes draw into the fill pass, so the external outline and shadow follow deformation unchanged. The fill bounding box must include skinned vertex bounds.
- **Intrinsic lines:** the part's line image is drawn with the same mesh and UVs in the colour pass.
- **Risk:** medium-low. The main risks are D2D/D3D ordering on the shared device (every D3D state is set explicitly per draw) and shader bytecode ownership. Choosing between build-time `fxc` and committed bytecode is decision D6.

---

## 12. Laptop / prop / contact architecture

- **The laptop is a prop rig, not body bones.** Today `laptop` is a child of `hips`. Proposed:
  - a separate prop root (`laptop_root → laptop_screen`);
  - mounted to the character through a **mount transform** that is switchable (lap / held / absent);
  - ideally expressed as a constraint relation between the prop root and a body anchor rather than parenting.
- **Draw order stays global:** body and prop slots interleave in one z list, as they do today:

      far arm < body < lid inner < near arm < lid < far leg < base < near leg < head < eyes

  The hierarchy never decides it (S5, Planner §2.3/§9).
- **Contacts are data:**
  - chin↔lid, paw↔lid and foot↔base become class B pairs (§9);
  - later they become IK targets: paw to a lid/keyboard point, and the head to a chin contact within a tolerance.
- **Body under the laptop must be complete** (§6), because a prop that can move or disappear exposes it.

---

## 13. Future IK / physics compatibility (architecture only)

| Element | Compatible? | Condition |
|---|---|---|
| Canonical-seated bind pose | Yes | Linear blend skinning works from any bind. Physics or IK drives bones, and meshes follow |
| Bone hierarchy (root → hips → body → chest → neck/head, limbs from chest/hips) | Yes | Needs bone **length/tip** data for IK and physics rotation (Spine physics warns about zero-length bones, S13) |
| Local meshes | Yes within their verified ranges | Beyond them, a physics pose must clamp to joint limits (verified range = hard limit, desired = soft) or swap to a substitution family (dangling) |
| Constraints | Yes | Add joint limits and IK targets as data; animation → constraints → physics offsets → skinning, in that order (as in S13) |
| Prop separation | Required | A held or dropped laptop needs the prop rig of §12 |
| Event-driven rendering | Yes | Physics activity temporarily requires continuous stepping; the idle policy applies whenever the simulation is at rest |

None of the recommendations here block active ragdoll. The seated-only rigid decomposition would.

---

## 14. Canonical seated-pose reconstruction verification

These checks apply to the M2 master rendered at bind, and to any future non-bind master.

| Metric | Definition | Proposed default target |
|---|---|---|
| Silhouette IoU | IoU of alpha>0.5 masks vs. `assets/mascot.png`, final composite including runtime outline | ≥ 0.98 (the current v0.2 rest differs by ≈0.48% of pixels, so ≥0.98 is attainable) |
| Landmark error | Pixel distance for named landmarks (eye centres, nose tip, ear tip, chin point, shoulder, paw, laptop corners ×4, hip, feet, tail tip), annotated once on the canonical image and located via part-local points on the master | eyes/nose ≤ 6 px; chin/ears/laptop ≤ 10 px; limbs/tail ≤ 15 px (at 1254 px) |
| Colour regions | Per semantic region, mean ΔE2000 of albedo | ≤ 3 |
| Line art | Chamfer distance between dark-line skeletons, per class | mean ≤ 3 px, p95 ≤ 8 px |
| Perceptual | SSIM on luminance over the full figure and per ROI | ≥ 0.95 full, ≥ 0.90 per ROI |
| Visual review | Side-by-side and flicker comparison at 1× and 4× | mandatory; any identity change is escalated |

A single scalar such as "90% similar" is not proposed. The table is the definition. Target values are decision D4.

---

## 15. KEEP / ADAPT / RETIRE / RESEARCH ONLY

Several KEEP/ADAPT items (`mascotctl`, `mascot-qa`, `qa.json`) exist only in the paused, **uncommitted** Windows working state. They are classified here as architecture, not as landed code.

| Artifact | Class | Note |
|---|---|---|
| Skeleton concepts (hierarchy, rest transforms, semantic pivots) | KEEP | Bones gain length/tip later |
| Clip schema 0.3 (compact authoring) | KEEP | Unaffected by art changes |
| `Attachment::Mesh` | ADAPT | Validation gaps and shared fill/line geometry (§8.1) |
| `skin_mesh` | KEEP | Sufficient 2D linear blend skinning |
| Runtime external-outline pipeline | KEEP | Becomes mesh-aware via the fill pass |
| Rigid sprite attachments | ADAPT | Still correct for rigid sockets and hinges (ears, head, paws, laptop) |
| Hidden patch generation (`tools/rig_art` extension/patch logic) | RETIRE | Replaced by drawn hidden art |
| Junction morphology / global junction regions | RETIRE | Planner §15 |
| Pivot heuristics (nearest-fill intersection) | RETIRE | Pivots become authored per joint contract. The chin-contact head pivot stays a hypothesis |
| Detector rules (frame artifact detector) | ADAPT | Keep signals; add mesh QA (§8.4) and class-B awareness; drop seated-specific allowances |
| Art-readiness tooling (sweeps, stress, zoom, montage) | KEEP / extend | Add `art joint` / `art readiness` (§16) |
| Current provisional safe ranges | RESEARCH ONLY | Diagnostics of the seated decomposition |
| Current seated part decomposition (`rig-v0.2/parts/*`) | RESEARCH ONLY → RETIRE on master adoption | Serves as the baseline for Prototypes A/B |
| `tools/rig_art/build_rig_v02.py` decomposition | RETIRE on master adoption | A derived-parts exporter from the layered master replaces it |
| Joint guards | ADAPT | Become part of the joint contracts |
| Neutral generated T-pose reference | RESEARCH ONLY | Non-canonical |

---

## 16. Tooling gaps

1. `mascotctl art joint <id> [--sheet|--sweep]` and `mascotctl art readiness [--json]` (Planner §13). There is no joint-contract sheet yet.
2. Joint-contract data integrated into the rig (no second schema): deformation class, desired vs. verified range, contact pairs, allowed internal lines.
3. Mesh tooling:
   - a deterministic mesh/weight generator (hull + feature loops + band weights);
   - `mascotctl rig mesh <id>` summary;
   - weight heatmap and wireframe render;
   - mesh QA (inversion, σ bounds, coverage).
4. CPU reference rasterizer for meshes (QA oracle; Prototype B harness).
5. Line-ownership inventory: a per-stroke class A/B/C map with owner/partner, and a check that no unclassified dark component exists in the master.
6. Contact-line preview: render V0/V1/V2 per pair and pose.
7. Reconstruction metrics: silhouette IoU, landmark table, ΔE per region, line chamfer, SSIM. SSIM and the landmark tooling do not exist yet.
8. Layered-master exporter: master layers → runtime `parts/*` + rig attachment data, replacing the seated decomposition build.
9. `mascotctl gate task|review` receipt (outstanding from WIN-002).
10. Performance: `qa stress` ≈12.4 s and `qa clip stretch --no-video` ≈7.5 s exceed targets (paused WIN-002 finding). This is still open.

---

## 17. Later prototype specifications (not implemented)

Both prototypes are built outside production behavior: behind a `mascotctl` research subcommand or feature, with outputs under `target/` until Planner approves adoption.

### Prototype A — dynamic contact line

- **Question:**
  - Can class B contact lines (chin↔lid, near paw↔lid) be produced by the composition rule of §9.2 so that they stay continuous and correctly owned at motion extremes?
  - Which line source (V1 owner stroke vs. V2 generated ring) gives acceptable rest fidelity?
- **Inputs:**
  - rig v0.2 parts `head.*`, `lid.*`, `lid_inner.*`, `arm_near.*` (HEAD versions);
  - `assets/mascot.png`;
  - the contact strokes extracted from the source in F-local space: chin contour over the lid, arm/paw contour over the lid.
- **Region:** contact ROIs around canvas (800,610) for the chin and (650–720, 640–800) for the paw.
- **Variants:**
  - V0: current baked lines (baseline);
  - V1: F-owned stroke texture clipped to coverage(B), drawn under colour(F);
  - V2: `dilate(alpha_F, w) ∩ edge_mask_F ∩ coverage(B)`, with `w` = outline stroke width and the edge mask derived from the rest stroke. Its taper band is tested at stroke ends.
- **Poses:**
  - head −10, −5, 0, +2, +5, +10 (+5/+10 diagnostic beyond the current clean range);
  - arm_near −15…+15 in 5° steps;
  - one combined pose.
- **Metrics per variant and pose:**
  - rest ROI diff vs. canonical: fraction of pixels with Δ > 32/255, and line-skeleton chamfer;
  - count of dark-line endpoints in the ROI vs. rest;
  - stroke-width p10/p50/p90 along the contact vs. rest;
  - orphan dark components (dark components not adjacent to the owner fill);
  - existing detector ERROR/WARN in the ROI;
  - render-time delta per frame (`MASCOT_RENDER_PROF`).
- **Visual evidence:** a V0/V1/V2 × pose grid at 1× and 4× crops, plus a previous/current/next strip for one sweep.
- **Failure criteria (either variant):**
  - rest ROI diff worse than V0;
  - any raw cut, ledge or orphan fragment at 4× within the tested range;
  - a line visible over the background (doubled with class A);
  - render-time delta > 1 ms per frame at 1×.
- **Scope:** renderer composition hook for one masked draw or one F-only dilation per pair; pair policy data; extraction of rest strokes into F-local masks; `mascotctl` preview command. No production art change.

### Prototype B — continuous-anatomy weighted mesh (hip / near thigh)

- **Question:** can a local weighted mesh extend the clean hip range from the measured rigid −4..0° to at least ±10° (target ±15°) with continuous rump→thigh flow and σ within the §8.4 bounds?
- **Why hip:** the strongest measured rigid failure. The rump contour is the canonical example (Planner §17.4/§17.8).
- **Inputs:**
  - rig v0.2 `leg_near.fill/line` and the rump region from `body.fill/line` in the transition band, composed at rest into one continuous hip texture;
  - body slot unchanged underneath;
  - bones `hips`, `leg_near_upper`.
- **Mesh:**
  - hull in transparent padding;
  - transition band of 0.45 × thigh width centred on the hip pivot;
  - edge loops at about 35 px spacing in the band, coarse elsewhere;
  - no self-overlap;
  - deterministic smoothstep weights, at most 2 influences.
- **Renderer:** the CPU reference rasterizer (R4) in the prototype harness. The GPU path is a separate later task.
- **Comparisons:** rigid v0.2 vs. mesh at −15, −10, −5, 0, +5, +10, +15.
- **Metrics:**
  - rest identity vs. rigid rest (§8.4);
  - per-triangle σ1, σ2, det over the sweep;
  - inversion and degenerate counts;
  - outline tangent-turn maximum along the rump→thigh contour (a kink measure; rigid is expected to show a corner);
  - detector findings in the `hip_near` ROI;
  - CPU skinning time and raster time per frame.
- **Visual evidence:** sweep sheet rigid vs. mesh, 4× and 8× hip crops, wireframe and weight-heatmap overlays.
- **Failure criteria:**
  - inversion or σ outside bounds within ±10°;
  - stroke-width change > 15% on line-covered triangles;
  - a texture hole or exposed body-cut;
  - no improvement in the clean range over rigid.
- **Scope:** mesh generator and QA (library-level, deterministic), CPU rasterizer, a hip mesh data file under `target/`, and a `mascotctl` research command. No renderer production change and no rig.json change.

### Prototype C — runtime top-light shading (optional; needs Planner approval, D2)

- **Question:** does `inset ∩ screen half-plane` reproduce the canonical arm, thigh and foot highlights at rest and keep them plausible under rotation?
- **Metrics:** rest ΔE and highlight-region IoU vs. canonical per group; visual review at ±15°.
- **Failure:** highlight IoU < 0.85 at rest, or visibly off-model shapes.

---

## 18. Proposed implementation phases (each requires explicit Planner authorization)

| Phase | Content | Exit criteria |
|---|---|---|
| P0 | Planner/user decisions D1–D7 (§19) | Written decisions |
| P1 | Prototypes A and B (C if approved); report | Prototype reports with the metrics above |
| P2 | Model/renderer: mesh draw-item path; validation hard cut (no silent renormalization); shared fill/line mesh geometry; D3D11 mesh pass (R1); class B composition, if Prototype A passes | Unit tests + bind-identity render + mesh QA |
| P3 | Layered art master (M2) per §6, with line-ownership metadata; exporter replacing the seated decomposition | Reconstruction metrics §14 + visual review |
| P4 | Joint contracts, static sweeps, verified safe ranges, `art readiness` | ART_READY per joint |
| P5 | Clips regenerated against the ART_READY rig; frame QA; evidence; Gates 1–20 | WIN-002 completion criteria |

---

## 19. Open Planner/user decisions

| # | Decision | Agent recommendation |
|---|---|---|
| D1 | Bind pose: canonical seated 3/4 (M2) vs. neutral T/A-pose master | M2, with substitution families later (§4–§5) |
| D2 | Runtime top-light shading vs. baked highlights | Prototype C first; runtime if it passes |
| D3 | Identity of anatomy the canonical image hides: belly colour/marking, far-arm/paw shape, toe lines | Owner decision; the neutral reference's cream belly is **not** assumed |
| D4 | Reconstruction targets (§14 values) | Accept the defaults or set product values |
| D5 | Who produces the layered master: human artist, agent redraw, or generated-then-edited | Owner decision; affects P3 scope and licensing/provenance |
| D6 | Shader bytecode ownership: build-time `fxc` vs. committed precompiled bytecode | Committed bytecode + source + regeneration command (no build-time SDK dependency) |
| D7 | Whether the prototype phase may add research-only `mascotctl` subcommands to the product tree, or must stay out-of-tree | In-tree behind a `research` subcommand group, removed or promoted after decision |
