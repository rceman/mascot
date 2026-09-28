//! Direct2D rig renderer.
//!
//! Per frame (all passes on the GPU, no CPU pixel work):
//!
//! 1. `color`  : every visible attachment (fills and internal line art) in z order
//! 2. `fill`   : fill attachments only -> composited mascot fill
//! 3. `black`  : ColorMatrix(fill) = (0, 0, 0, fill_alpha) silhouette
//! 4. `outline`: dilation of the silhouette by the outline radius, computed as the
//!    Minkowski sum of two rings (ring(R/2) + ring(R/2) = disc(R)) with
//!    `D2D1_PRIMITIVE_BLEND_MAX` bitmap stamps. Line art never enters this mask.
//! 5. target   : background -> runtime shadow (from `outline`) -> outline -> color
//!    -> optional debug overlay.

use crate::image::RgbaImage;
use mascot_animation::{Affine, DrawItem, Rig, Role};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::{Interface, Result, w};
use windows_numerics::{Matrix3x2, Vector2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// GPU (falls back to WARP if no hardware device can be created).
    Hardware,
    /// Microsoft Basic Render Driver: deterministic across machines; used for evidence.
    Warp,
}

/// Canvas pixels -> target pixels: `p * scale + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub scale: f32,
    pub offset: [f32; 2],
}

impl View {
    pub fn affine(&self) -> Affine {
        Affine::translate(self.offset[0], self.offset[1]).mul(Affine::scale(self.scale, self.scale))
    }
    /// Fits a canvas of `canvas` pixels into `target` with `margin` pixels padding.
    pub fn fit(canvas: [u32; 2], target: [u32; 2], margin: f32) -> View {
        let s = ((target[0] as f32 - 2.0 * margin) / canvas[0] as f32)
            .min((target[1] as f32 - 2.0 * margin) / canvas[1] as f32);
        View {
            scale: s,
            offset: [
                (target[0] as f32 - canvas[0] as f32 * s) / 2.0,
                (target[1] as f32 - canvas[1] as f32 * s) / 2.0,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    /// Contact shadow: silhouette squashed onto the floor line and blurred.
    pub contact_opacity: f32,
    pub contact_squash: f32,
    pub contact_blur_canvas_px: f32,
    /// Drop shadow: blurred silhouette offset down/right.
    pub drop_opacity: f32,
    pub drop_offset_canvas_px: [f32; 2],
    pub drop_blur_canvas_px: f32,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        ShadowStyle {
            contact_opacity: 0.30,
            contact_squash: 0.10,
            contact_blur_canvas_px: 14.0,
            drop_opacity: 0.18,
            drop_offset_canvas_px: [10.0, 14.0],
            drop_blur_canvas_px: 10.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderOptions {
    pub outline: bool,
    pub shadow: bool,
    pub shadow_style: ShadowStyle,
    /// Straight RGBA background; `None` = transparent.
    pub background: Option<[f32; 4]>,
    pub bones: bool,
    pub pivots: bool,
    pub bounds: bool,
    pub labels: bool,
    pub selected_bone: Option<usize>,
    /// Canvas-space floor line y for the contact shadow (defaults to the root pivot).
    pub floor_y: Option<f32>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            outline: true,
            shadow: false,
            shadow_style: ShadowStyle::default(),
            background: None,
            bones: false,
            pivots: false,
            bounds: false,
            labels: false,
            selected_bone: None,
            floor_y: None,
        }
    }
}

/// Intermediate layers, read back for automated analysis.
pub struct Layers {
    pub final_image: RgbaImage,
    /// Straight RGBA of the fill+line composite (no outline, no shadow).
    pub color: RgbaImage,
    /// Fill silhouette alpha (what the outline is generated from).
    pub fill_alpha: Vec<u8>,
    /// Outline mask alpha (dilated silhouette).
    pub outline_alpha: Vec<u8>,
}

struct Scratch {
    size: [u32; 2],
    color: ID2D1Bitmap1,
    fill: ID2D1Bitmap1,
    black: ID2D1Bitmap1,
    ring: ID2D1Bitmap1,
    outline: ID2D1Bitmap1,
}

pub struct Renderer {
    pub kind: DeviceKind,
    pub d3d: ID3D11Device,
    pub factory: ID2D1Factory1,
    pub device: ID2D1Device,
    pub ctx: ID2D1DeviceContext,
    dwrite: IDWriteFactory,
    label_format: IDWriteTextFormat,
    bitmaps: Vec<Option<ID2D1Bitmap1>>,
    scratch: Option<Scratch>,
    color_matrix: ID2D1Effect,
    contact_affine: ID2D1Effect,
    contact_shadow: ID2D1Effect,
    drop_shadow: ID2D1Effect,
}

fn m3x2(a: Affine) -> Matrix3x2 {
    Matrix3x2 { M11: a.a, M12: a.b, M21: a.c, M22: a.d, M31: a.tx, M32: a.ty }
}

fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

pub fn bitmap_props(options: D2D1_BITMAP_OPTIONS) -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: options,
        colorContext: std::mem::ManuallyDrop::new(None),
    }
}

pub fn create_d3d_device(kind: DeviceKind) -> Result<(ID3D11Device, DeviceKind)> {
    let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0];
    let try_create = |driver: D3D_DRIVER_TYPE| -> Result<ID3D11Device> {
        let mut dev = None;
        unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                None,
            )?;
        }
        Ok(dev.expect("D3D11CreateDevice returned no device"))
    };
    match kind {
        DeviceKind::Warp => Ok((try_create(D3D_DRIVER_TYPE_WARP)?, DeviceKind::Warp)),
        DeviceKind::Hardware => match try_create(D3D_DRIVER_TYPE_HARDWARE) {
            Ok(d) => Ok((d, DeviceKind::Hardware)),
            Err(_) => Ok((try_create(D3D_DRIVER_TYPE_WARP)?, DeviceKind::Warp)),
        },
    }
}

impl Renderer {
    pub fn new(kind: DeviceKind) -> Result<Self> {
        let (d3d, kind) = create_d3d_device(kind)?;
        unsafe {
            let factory: ID2D1Factory1 = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS { debugLevel: D2D1_DEBUG_LEVEL_NONE }),
            )?;
            let dxgi: IDXGIDevice = d3d.cast()?;
            let device = factory.CreateDevice(&dxgi)?;
            let ctx = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let label_format = dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                11.0,
                w!("en-us"),
            )?;
            let color_matrix = ctx.CreateEffect(&CLSID_D2D1ColorMatrix)?;
            // [R G B A 1] x M: only alpha survives -> black silhouette. Alpha is
            // sharpened (a' = 2a - 0.5, clamped) so faint feathered fill never
            // grows a translucent outline blob; AA edges shift by < 0.5 px.
            let mut m = [0f32; 20];
            m[15] = 2.0;
            m[19] = -0.5;
            color_matrix.SetValue(D2D1_COLORMATRIX_PROP_COLOR_MATRIX.0 as u32, D2D1_PROPERTY_TYPE_MATRIX_5X4, &f32_bytes(&m))?;
            color_matrix.SetValue(D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT.0 as u32, D2D1_PROPERTY_TYPE_BOOL, &1u32.to_ne_bytes())?;
            let contact_affine = ctx.CreateEffect(&CLSID_D2D12DAffineTransform)?;
            let contact_shadow = ctx.CreateEffect(&CLSID_D2D1Shadow)?;
            contact_shadow.SetInput(0, &contact_affine.GetOutput()?, true);
            let drop_shadow = ctx.CreateEffect(&CLSID_D2D1Shadow)?;
            Ok(Renderer {
                kind,
                d3d,
                factory,
                device,
                ctx,
                dwrite,
                label_format,
                bitmaps: Vec::new(),
                scratch: None,
                color_matrix,
                contact_affine,
                contact_shadow,
                drop_shadow,
            })
        }
    }

    /// Uploads every sprite of `rig` as a premultiplied GPU bitmap.
    pub fn load_rig(&mut self, rig: &Rig) -> std::result::Result<(), String> {
        self.bitmaps.clear();
        for slot in &rig.slots {
            let bmp = match &slot.attachment {
                mascot_animation::Attachment::Sprite(s) => {
                    let img = RgbaImage::load(&rig.dir.join(&s.image))?;
                    if [img.width, img.height] != s.size {
                        return Err(format!("{}: size {}x{} != rig {:?}", s.image, img.width, img.height, s.size));
                    }
                    let bgra = img.to_premultiplied_bgra();
                    let bmp = unsafe {
                        self.ctx.CreateBitmap(
                            D2D_SIZE_U { width: img.width, height: img.height },
                            Some(bgra.as_ptr() as *const _),
                            img.width * 4,
                            &bitmap_props(D2D1_BITMAP_OPTIONS_NONE),
                        )
                    }
                    .map_err(|e| format!("CreateBitmap {}: {e}", s.image))?;
                    Some(bmp)
                }
                mascot_animation::Attachment::Mesh(_) => None,
            };
            self.bitmaps.push(bmp);
        }
        Ok(())
    }

    fn ensure_scratch(&mut self, size: [u32; 2]) -> Result<()> {
        if self.scratch.as_ref().is_some_and(|s| s.size == size) {
            return Ok(());
        }
        let mk = || unsafe {
            self.ctx.CreateBitmap(
                D2D_SIZE_U { width: size[0].max(1), height: size[1].max(1) },
                None,
                0,
                &bitmap_props(D2D1_BITMAP_OPTIONS_TARGET),
            )
        };
        self.scratch = Some(Scratch {
            size,
            color: mk()?,
            fill: mk()?,
            black: mk()?,
            ring: mk()?,
            outline: mk()?,
        });
        Ok(())
    }

    pub fn create_target_bitmap(&self, size: [u32; 2]) -> Result<ID2D1Bitmap1> {
        unsafe {
            self.ctx.CreateBitmap(
                D2D_SIZE_U { width: size[0], height: size[1] },
                None,
                0,
                &bitmap_props(D2D1_BITMAP_OPTIONS_TARGET),
            )
        }
    }

    fn draw_items(&self, items: &[DrawItem], view: Affine, filter: impl Fn(Role) -> bool) {
        unsafe {
            for it in items.iter().filter(|i| filter(i.role)) {
                let Some(bmp) = &self.bitmaps[it.slot] else { continue };
                self.ctx.SetTransform(&m3x2(view.mul(it.transform)));
                self.ctx.DrawBitmap(bmp, None, it.opacity.clamp(0.0, 1.0), D2D1_INTERPOLATION_MODE_LINEAR, None, None);
            }
            self.ctx.SetTransform(&m3x2(Affine::IDENTITY));
        }
    }

    /// Stamps `src` onto the current target at offsets on a circle of radius `r`
    /// (with MAX blending: union of translated copies).
    fn stamp_ring(&self, src: &ID2D1Bitmap1, size: [u32; 2], r: f32) {
        let n = ((std::f32::consts::TAU * r / 0.75).ceil() as usize).max(8);
        self.stamp_offsets(src, size, r, n);
    }

    fn stamp_offsets(&self, src: &ID2D1Bitmap1, size: [u32; 2], r: f32, n: usize) {
        let (w, h) = (size[0] as f32, size[1] as f32);
        unsafe {
            for i in 0..n {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                let (dx, dy) = (r * t.cos(), r * t.sin());
                let rect = D2D_RECT_F { left: dx, top: dy, right: w + dx, bottom: h + dy };
                self.ctx.DrawBitmap(src, Some(&rect), 1.0, D2D1_INTERPOLATION_MODE_LINEAR, None, None);
            }
        }
    }

    /// Renders one frame into `target` (a D2D target bitmap of `size` pixels).
    pub fn render(
        &mut self,
        target: &ID2D1Bitmap1,
        size: [u32; 2],
        rig: &Rig,
        items: &[DrawItem],
        world: &[Affine],
        view: View,
        opts: &RenderOptions,
    ) -> Result<()> {
        self.ensure_scratch(size)?;
        let s = self.scratch.as_ref().unwrap();
        let (sc_color, sc_fill, sc_black, sc_ring, sc_outline) =
            (s.color.clone(), s.fill.clone(), s.black.clone(), s.ring.clone(), s.outline.clone());
        let v = view.affine();
        let transparent = color(0.0, 0.0, 0.0, 0.0);
        unsafe {
            let ctx = &self.ctx;
            // 1. colour composite (fills + internal line art, z order)
            ctx.SetTarget(&sc_color);
            ctx.BeginDraw();
            ctx.Clear(Some(&transparent));
            self.draw_items(items, v, |_| true);
            ctx.EndDraw(None, None)?;
            let radius = rig.outline_radius() * view.scale;
            let need_mask = opts.outline || opts.shadow;
            if need_mask {
                // 2. fill silhouette
                ctx.SetTarget(&sc_fill);
                ctx.BeginDraw();
                ctx.Clear(Some(&transparent));
                self.draw_items(items, v, |r| r == Role::Fill);
                ctx.EndDraw(None, None)?;
                // 3. black silhouette
                self.color_matrix.SetInput(0, &sc_fill.cast::<ID2D1Image>()?, true);
                ctx.SetTarget(&sc_black);
                ctx.BeginDraw();
                ctx.Clear(Some(&transparent));
                ctx.DrawImage(
                    &self.color_matrix.GetOutput()?,
                    None,
                    None,
                    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                    D2D1_COMPOSITE_MODE_SOURCE_OVER,
                );
                ctx.EndDraw(None, None)?;
                // 4. dilation: ring(R/2) (+) ring(R/2) == disc(R)
                ctx.SetTarget(&sc_ring);
                ctx.BeginDraw();
                ctx.Clear(Some(&transparent));
                ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_MAX);
                self.stamp_ring(&sc_black, size, radius * 0.5);
                ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
                ctx.EndDraw(None, None)?;
                ctx.SetTarget(&sc_outline);
                ctx.BeginDraw();
                ctx.Clear(Some(&transparent));
                ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_MAX);
                self.stamp_ring(&sc_ring, size, radius * 0.5);
                ctx.DrawBitmap(&sc_black, None, 1.0, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, None, None);
                ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
                ctx.EndDraw(None, None)?;
            }
            // 5. final
            ctx.SetTarget(target);
            ctx.BeginDraw();
            match opts.background {
                Some(bg) => ctx.Clear(Some(&color(bg[0] * bg[3], bg[1] * bg[3], bg[2] * bg[3], bg[3]))),
                None => ctx.Clear(Some(&transparent)),
            }
            if opts.shadow {
                self.draw_shadow(&sc_outline, rig, world, view, &opts.shadow_style, opts.floor_y)?;
            }
            if opts.outline {
                ctx.DrawBitmap(&sc_outline, None, 1.0, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, None, None);
            }
            ctx.DrawBitmap(&sc_color, None, 1.0, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, None, None);
            if opts.bones || opts.pivots || opts.bounds {
                self.draw_overlay(rig, items, world, view, opts)?;
            }
            ctx.EndDraw(None, None)?;
        }
        Ok(())
    }

    fn draw_shadow(
        &self,
        silhouette: &ID2D1Bitmap1,
        rig: &Rig,
        world: &[Affine],
        view: View,
        style: &ShadowStyle,
        floor_y: Option<f32>,
    ) -> Result<()> {
        let floor_canvas = floor_y.unwrap_or_else(|| world[0].apply([0.0, 0.0])[1]);
        let floor = floor_canvas * view.scale + view.offset[1];
        let _ = rig;
        unsafe {
            let sil: ID2D1Image = silhouette.cast()?;
            if style.contact_opacity > 0.0 {
                // squash the current silhouette onto the floor line: y' = floor + (y - floor) * k
                let k = style.contact_squash;
                let m = Matrix3x2 { M11: 1.0, M12: 0.0, M21: 0.0, M22: k, M31: 0.0, M32: floor * (1.0 - k) };
                self.contact_affine.SetInput(0, &sil, true);
                self.contact_affine.SetValue(
                    D2D1_2DAFFINETRANSFORM_PROP_TRANSFORM_MATRIX.0 as u32,
                    D2D1_PROPERTY_TYPE_MATRIX_3X2,
                    &f32_bytes(&[m.M11, m.M12, m.M21, m.M22, m.M31, m.M32]),
                )?;
                self.contact_shadow.SetValue(
                    D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                    D2D1_PROPERTY_TYPE_FLOAT,
                    &f32_bytes(&[style.contact_blur_canvas_px * view.scale / 3.0]),
                )?;
                self.contact_shadow.SetValue(
                    D2D1_SHADOW_PROP_COLOR.0 as u32,
                    D2D1_PROPERTY_TYPE_VECTOR4,
                    &f32_bytes(&[0.0, 0.0, 0.0, style.contact_opacity]),
                )?;
                self.ctx.DrawImage(
                    &self.contact_shadow.GetOutput()?,
                    None,
                    None,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    D2D1_COMPOSITE_MODE_SOURCE_OVER,
                );
            }
            if style.drop_opacity > 0.0 {
                self.drop_shadow.SetInput(0, &sil, true);
                self.drop_shadow.SetValue(
                    D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                    D2D1_PROPERTY_TYPE_FLOAT,
                    &f32_bytes(&[style.drop_blur_canvas_px * view.scale / 3.0]),
                )?;
                self.drop_shadow.SetValue(
                    D2D1_SHADOW_PROP_COLOR.0 as u32,
                    D2D1_PROPERTY_TYPE_VECTOR4,
                    &f32_bytes(&[0.0, 0.0, 0.0, style.drop_opacity]),
                )?;
                // the drop offset follows the rig's facing so a mirrored mascot casts a mirrored shadow
                let facing = if world[0].det() < 0.0 { -1.0 } else { 1.0 };
                let off = Vector2 {
                    X: style.drop_offset_canvas_px[0] * view.scale * facing,
                    Y: style.drop_offset_canvas_px[1] * view.scale,
                };
                self.ctx.DrawImage(
                    &self.drop_shadow.GetOutput()?,
                    Some(&off),
                    None,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    D2D1_COMPOSITE_MODE_SOURCE_OVER,
                );
            }
        }
        Ok(())
    }

    fn draw_overlay(&self, rig: &Rig, items: &[DrawItem], world: &[Affine], view: View, opts: &RenderOptions) -> Result<()> {
        let v = view.affine();
        unsafe {
            let ctx = &self.ctx;
            let brush = |c: D2D1_COLOR_F| ctx.CreateSolidColorBrush(&c, None);
            let bone_b = brush(color(0.05, 0.55, 1.0, 0.95))?;
            let sel_b = brush(color(1.0, 0.1, 0.35, 1.0))?;
            let piv_b = brush(color(1.0, 1.0, 1.0, 1.0))?;
            let piv_o = brush(color(0.9, 0.0, 0.6, 1.0))?;
            let bnd_b = brush(color(0.1, 0.75, 0.2, 0.9))?;
            let txt_b = brush(color(0.05, 0.05, 0.15, 1.0))?;
            let txt_bg = brush(color(1.0, 1.0, 1.0, 0.8))?;
            if opts.bounds {
                for it in items {
                    let slot = &rig.slots[it.slot];
                    if slot.role != Role::Fill {
                        continue;
                    }
                    if let mascot_animation::Attachment::Sprite(sp) = &slot.attachment {
                        let m = v.mul(it.transform);
                        let c = [[0.0, 0.0], [sp.size[0] as f32, 0.0], [sp.size[0] as f32, sp.size[1] as f32], [0.0, sp.size[1] as f32]]
                            .map(|p| m.apply(p));
                        for i in 0..4 {
                            let (a, b) = (c[i], c[(i + 1) % 4]);
                            ctx.DrawLine(Vector2 { X: a[0], Y: a[1] }, Vector2 { X: b[0], Y: b[1] }, &bnd_b, 1.0, None);
                        }
                    }
                }
            }
            let pts: Vec<[f32; 2]> = world.iter().map(|w| v.apply(w.apply([0.0, 0.0]))).collect();
            if opts.bones {
                for (i, b) in rig.skeleton.bones.iter().enumerate() {
                    if let Some(p) = b.parent {
                        let (a, c) = (pts[p], pts[i]);
                        let br = if opts.selected_bone == Some(i) { &sel_b } else { &bone_b };
                        ctx.DrawLine(Vector2 { X: a[0], Y: a[1] }, Vector2 { X: c[0], Y: c[1] }, br, 2.0, None);
                    }
                }
            }
            if opts.pivots {
                for (i, p) in pts.iter().enumerate() {
                    let sel = opts.selected_bone == Some(i);
                    let r = if sel { 6.0 } else { 4.0 };
                    let e = D2D1_ELLIPSE { point: Vector2 { X: p[0], Y: p[1] }, radiusX: r, radiusY: r };
                    ctx.FillEllipse(&e, &piv_b);
                    ctx.DrawEllipse(&e, if sel { &sel_b } else { &piv_o }, 2.0, None);
                }
            }
            if opts.labels {
                for (i, b) in rig.skeleton.bones.iter().enumerate() {
                    let p = pts[i];
                    let text: Vec<u16> = b.id.encode_utf16().collect();
                    let wdt = 7.0 * text.len() as f32 + 6.0;
                    let rect = D2D_RECT_F { left: p[0] + 6.0, top: p[1] - 7.0, right: p[0] + 6.0 + wdt, bottom: p[1] + 8.0 };
                    ctx.FillRectangle(&rect, &txt_bg);
                    ctx.DrawText(&text, &self.label_format, &rect, &txt_b, D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE_NATURAL);
                }
            }
        }
        Ok(())
    }

    /// Copies a target bitmap to CPU memory as straight RGBA.
    pub fn read_back(&self, bitmap: &ID2D1Bitmap1, size: [u32; 2]) -> Result<RgbaImage> {
        unsafe {
            let cpu = self.ctx.CreateBitmap(
                D2D_SIZE_U { width: size[0], height: size[1] },
                None,
                0,
                &bitmap_props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW),
            )?;
            cpu.CopyFromBitmap(None, bitmap, None)?;
            let map = cpu.Map(D2D1_MAP_OPTIONS_READ)?;
            let bits = std::slice::from_raw_parts(map.bits, map.pitch as usize * size[1] as usize);
            let img = RgbaImage::from_premultiplied_bgra(size[0], size[1], map.pitch as usize, bits);
            cpu.Unmap()?;
            Ok(img)
        }
    }

    /// Offscreen render with read-back of the final image and analysis layers.
    pub fn render_layers(
        &mut self,
        size: [u32; 2],
        rig: &Rig,
        items: &[DrawItem],
        world: &[Affine],
        view: View,
        opts: &RenderOptions,
    ) -> Result<Layers> {
        let target = self.create_target_bitmap(size)?;
        let mut o = opts.clone();
        o.outline = true;
        self.render(&target, size, rig, items, world, view, &o)?;
        let color_img = {
            let s = self.scratch.as_ref().unwrap();
            self.read_back(&s.color.clone(), size)?
        };
        let (fill_alpha, outline_alpha) = {
            let s = self.scratch.as_ref().unwrap();
            let (f, ol) = (s.fill.clone(), s.outline.clone());
            (self.read_back(&f, size)?.alpha(), self.read_back(&ol, size)?.alpha())
        };
        if !opts.outline {
            self.render(&target, size, rig, items, world, view, opts)?;
        }
        let final_image = self.read_back(&target, size)?;
        Ok(Layers { final_image, color: color_img, fill_alpha, outline_alpha })
    }

    pub fn dwrite(&self) -> &IDWriteFactory {
        &self.dwrite
    }

    /// Lays out labelled images on a grid (evidence contact sheets).
    pub fn compose_sheet(
        &self,
        cells: &[(String, &RgbaImage)],
        cols: usize,
        cell: [u32; 2],
        label_h: u32,
        bg: [f32; 3],
    ) -> Result<RgbaImage> {
        let rows = cells.len().div_ceil(cols.max(1));
        let size = [cell[0] * cols as u32, (cell[1] + label_h) * rows as u32];
        let target = self.create_target_bitmap(size)?;
        unsafe {
            let fmt = self.dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                (label_h as f32 * 0.62).max(9.0),
                w!("en-us"),
            )?;
            let ctx = &self.ctx;
            ctx.SetTarget(&target);
            ctx.BeginDraw();
            ctx.Clear(Some(&color(bg[0], bg[1], bg[2], 1.0)));
            let txt = ctx.CreateSolidColorBrush(&color(0.1, 0.1, 0.12, 1.0), None)?;
            let grid = ctx.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.18), None)?;
            for (i, (label, img)) in cells.iter().enumerate() {
                let (cx, cy) = ((i % cols) as f32 * cell[0] as f32, (i / cols) as f32 * (cell[1] + label_h) as f32);
                let bgra = img.to_premultiplied_bgra();
                let bmp = ctx.CreateBitmap(
                    D2D_SIZE_U { width: img.width, height: img.height },
                    Some(bgra.as_ptr() as *const _),
                    img.width * 4,
                    &bitmap_props(D2D1_BITMAP_OPTIONS_NONE),
                )?;
                let s = (cell[0] as f32 / img.width as f32).min(cell[1] as f32 / img.height as f32);
                let (w, h) = (img.width as f32 * s, img.height as f32 * s);
                let dst = D2D_RECT_F {
                    left: cx + (cell[0] as f32 - w) / 2.0,
                    top: cy + label_h as f32 + (cell[1] as f32 - h) / 2.0,
                    right: cx + (cell[0] as f32 + w) / 2.0,
                    bottom: cy + label_h as f32 + (cell[1] as f32 + h) / 2.0,
                };
                ctx.DrawBitmap(&bmp, Some(&dst), 1.0, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, None, None);
                let text: Vec<u16> = label.encode_utf16().collect();
                let lr = D2D_RECT_F { left: cx + 6.0, top: cy + 2.0, right: cx + cell[0] as f32, bottom: cy + label_h as f32 };
                ctx.DrawText(&text, &fmt, &lr, &txt, D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE_NATURAL);
                let frame = D2D_RECT_F { left: cx + 0.5, top: cy + 0.5, right: cx + cell[0] as f32 - 0.5, bottom: cy + (cell[1] + label_h) as f32 - 0.5 };
                ctx.DrawRectangle(&frame, &grid, 1.0, None);
            }
            ctx.EndDraw(None, None)?;
        }
        self.read_back(&target, size)
    }
}
