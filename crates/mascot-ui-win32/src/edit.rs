//! Windowless RichEdit host: `ITextHost2` + `ITextServices2` with `TxDrawD2D`.
//!
//! The composer's editable text is the real msftedit engine hosted windowless
//! and rendered into our Direct2D frame, so it stays inside the single
//! transparent composition (and inside offscreen captures).
//!
//! IID notes: windows-rs declares `ITextHost`, `ITextHost2`, `ITextServices`
//! and `ITextServices2` with a zero IID (implement-only). The real interface
//! IDs only exist as data exports inside msftedit.dll. [`Msftedit`] loads them
//! and the host's `QueryInterface` answers them explicitly, otherwise msftedit
//! would never see `ITextHost2` and D2D mode would silently stay off.
//!
//! Units (verified by the DPI probes in `tests/edit_probe.rs`): the entire
//! host coordinate space is **DIP**. msftedit's D2D path draws its format
//! units 1:1 into the logical units of `TxDrawD2D` bounds — reporting a
//! device-px client rect makes it latch a per-object dpi at activation that
//! goes stale on scale change (`dpi_latch_matrix`). Keeping the client rect
//! in DIPs and `TxGetViewExtent` fixed at 96-dpi himetric makes the format
//! space scale-independent: no recreate is needed on DPI change.
//! `TxGetNaturalSize`/`TxGetExtent` are HIMETRIC (1/100 mm;
//! `himetric = dip * 2540/96`). `TxGetClientRect`/`TxInvalidateRect`/caret
//! coordinates are DIPs; `TxScreenToClient`/`TxClientToScreen` convert
//! screen px <-> DIP via the stored scale.
//!
//! Object layout note: msftedit assumes the host is a C++ single-inheritance
//! object — it calls `ITextHost2` methods through the pointer it got for
//! `ITextHost` (and vice versa). The host therefore carries ONE vtbl slot
//! (the full `ITextHost2` vtbl, OFFSET 0) and `QueryInterface` answers
//! `IUnknown`/`ITextHost`/`ITextHost2` all with `base+0`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::sync::OnceLock;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::Controls::EM_LIMITTEXT;
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

/// Win32 editor messages RichEdit understands that are missing from bindings.
const EN_CHANGE_CODE: u32 = 0x0300;
const EN_REQUESTRESIZE_CODE: u32 = 0x0701;
/// RichEdit requests host timers with small ids; offset so they never collide
/// with app timers when forwarded to `SetTimer`.
pub const EDIT_TIMER_BASE: usize = 0x4d00;

const fn colorref(c: [f32; 4]) -> COLORREF {
    COLORREF(
        ((c[0] * 255.0 + 0.5) as u32)
            | (((c[1] * 255.0 + 0.5) as u32) << 8)
            | (((c[2] * 255.0 + 0.5) as u32) << 16),
    )
}

/// msftedit.dll handle + resolved entry points / IIDs.
pub type CreateTextServicesFn =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;

pub struct Msftedit {
    pub module: HMODULE,
    create_text_services: CreateTextServicesFn,
    pub iid_text_services: GUID,
    pub iid_text_services2: GUID,
    pub iid_text_host: GUID,
    pub iid_text_host2: GUID,
    /// `IID_IRicheditWindowlessAccessibility` — exported from msftedit like
    /// the others (`windows` ships the interface with a zero GUID).
    pub iid_windowless_acc: Option<GUID>,
}

/// The exported `IID_IRicheditWindowlessAccessibility` GUID (msftedit).
pub fn iid_windowless_accessibility() -> Result<GUID> {
    Msftedit::load()?
        .iid_windowless_acc
        .ok_or_else(|| Error::new(E_POINTER, "no IID_IRicheditWindowlessAccessibility export"))
}

// A loaded module handle + entry points are immutable and safe to share.
unsafe impl Send for Msftedit {}
unsafe impl Sync for Msftedit {}

impl Msftedit {
    pub fn load() -> Result<&'static Msftedit> {
        static LIB: OnceLock<Result<Msftedit>> = OnceLock::new();
        match LIB.get_or_init(|| unsafe { Msftedit::load_impl() }) {
            Ok(l) => Ok(l),
            Err(e) => Err(Error::from_hresult(e.code())),
        }
    }

    unsafe fn load_impl() -> Result<Msftedit> {
        unsafe {
            // OleInitialize is required by windowless text services.
            let _ = OleInitialize(None);
            let module = LoadLibraryExW(w!("msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                .map_err(|e| Error::new(e.code(), "LoadLibraryExW(msftedit.dll)"))?;
            let data_iid = |name: &std::ffi::CStr| -> Result<GUID> {
                let p = GetProcAddress(module, windows::core::PCSTR(name.as_ptr() as *const u8));
                if p.is_none() {
                    return Err(Error::new(E_POINTER, "missing IID export"));
                }
                Ok(*(p.unwrap() as *const GUID))
            };
            let create = GetProcAddress(module, s!("CreateTextServices"));
            let create = create.ok_or_else(|| Error::new(E_POINTER, "no CreateTextServices"))?;
            Ok(Msftedit {
                module,
                create_text_services: std::mem::transmute::<*const u8, CreateTextServicesFn>(
                    create as *const u8,
                ),
                iid_text_services: data_iid(c"IID_ITextServices")?,
                iid_text_services2: data_iid(c"IID_ITextServices2")?,
                iid_text_host: data_iid(c"IID_ITextHost")?,
                iid_text_host2: data_iid(c"IID_ITextHost2")?,
                iid_windowless_acc: data_iid(c"IID_IRicheditWindowlessAccessibility").ok(),
            })
        }
    }
}

/// Host → app notifications collected while msftedit runs inside one of our
/// calls. The app drains them after every `Editor` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEvent {
    /// Text services invalidated the view (or part of it).
    Invalidate,
    /// EN_CHANGE: content or formatting changed.
    Change,
    /// Richedit asked the host for a timer (id, ms).
    SetTimer(u32, u32),
    KillTimer(u32),
    /// Richedit wants mouse capture set/released.
    Capture(bool),
    /// Caret geometry or visibility changed.
    Caret,
}

/// Everything `TxGet*` / caret / timers read back from the host.
pub struct HostShared {
    /// Owning window handle (IME context, timers, capture). May be default for
    /// offscreen-only editors.
    pub hwnd: HWND,
    /// Editor client rect in DIPs. msftedit's D2D path treats its "client
    /// units" as device-independent (format units draw 1:1 into logical DIPs),
    /// so the whole host coordinate space is DIP — the target DPI only
    /// matters when presenting. Verified by `dpi_latch_matrix` in
    /// `tests/edit_probe.rs`: a px-space client rect makes the text scale
    /// with the activation-time dpi latch, which can only be wrong at scale>1.
    pub client: RECT,
    /// DPI scale of the render target (px per DIP). Only used to convert
    /// client units <-> screen pixels for `TxScreenToClient`/`TxClientToScreen`.
    pub scale: f32,
    /// Property bits advertised via TxGetPropertyBits.
    pub bits: u32,
    /// Latest EN_REQUESTRESIZE size in client units (DIPs).
    pub natural_px: SIZE,
    /// Char/para format we hand back.
    pub cf: Box<CHARFORMATW>,
    pub pf: Box<PARAFORMAT>,
    /// Text colour (COLORREF) for the editor.
    pub fg: COLORREF,
    /// Selection colours.
    pub sel_bg: COLORREF,
    pub sel_fg: COLORREF,
    /// Caret state recorded from TxCreateCaret/TxShowCaret/TxSetCaretPos.
    pub caret_pos: POINT,
    pub caret_size: SIZE,
    pub caret_shown: bool,
    pub caret_created: bool,
    /// Richedit-requested timer ids currently installed.
    pub timers: BTreeSet<u32>,
    /// Pending host events, drained by `Editor::drain_events`.
    pub events: Vec<HostEvent>,
    /// Whether the editor is read-only (submitting state dims the text too).
    pub read_only: bool,
    /// Dim color applied to all runs (submitting state).
    pub dim: Option<[f32; 4]>,
    /// Content changed and the caller should re-measure.
    pub dirty: bool,
}

/// COM object handed to `CreateTextServices`. First two fields are the
/// interface slots msftedit may QI for; `Identity` for the generated vtable
/// shims is the whole box.
#[repr(C)]
struct HostBox {
    host: &'static ITextHost_Vtbl,
    host2: &'static ITextHost2_Vtbl,
    refs: Cell<u32>,
    state: RefCell<HostShared>,
}

// Single-inheritance layout (as in C++): slot 0 carries the full ITextHost2
// vtbl with OFFSET 0 so both ITextHost and ITextHost2 pointers are base+0.
static VT_HOST2: ITextHost2_Vtbl = ITextHost2_Vtbl::new::<HostBox, 0>();

impl IUnknownImpl for HostBox {
    type Impl = HostBox;
    fn get_impl(&self) -> &Self::Impl {
        self
    }
    fn get_impl_mut(&mut self) -> &mut Self::Impl {
        self
    }
    fn into_inner(self) -> Self::Impl {
        self
    }
    unsafe fn QueryInterface(&self, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
        unsafe {
            *out = std::ptr::null_mut();
            let lib = Msftedit::load().expect("msftedit");
            let base = self as *const HostBox as *mut c_void;
            if *iid == IUnknown::IID || *iid == lib.iid_text_host || *iid == lib.iid_text_host2 {
                // single-inheritance layout: one vtbl serves all three IIDs
                *out = base;
            } else {
                return E_NOINTERFACE;
            }
            self.AddRef();
            S_OK
        }
    }
    fn AddRef(&self) -> u32 {
        self.refs.set(self.refs.get() + 1);
        self.refs.get()
    }
    unsafe fn Release(this: *mut Self) -> u32 {
        unsafe {
            let b = &*this;
            let n = b.refs.get() - 1;
            b.refs.set(n);
            if n == 0 {
                drop(Box::from_raw(this));
            }
            n
        }
    }
    fn is_reference_count_one(&self) -> bool {
        self.refs.get() == 1
    }
    fn to_object(&self) -> ComObject<HostBox>
    where
        HostBox: ComObjectInner<Outer = HostBox>,
    {
        // never called: we manage the object manually
        unimplemented!("HostBox is not a ComObject")
    }
    unsafe fn GetTrustLevel(&self, value: *mut i32) -> HRESULT {
        unsafe {
            if !value.is_null() {
                *value = 0;
            }
        }
        S_OK
    }
}

impl ComObjectInner for HostBox {
    type Outer = HostBox;
    fn into_object(self) -> ComObject<Self> {
        // never called: the object is created by Box::into_raw in Editor::new
        unimplemented!("HostBox is constructed manually")
    }
}

impl HostBox {
    fn s(&self) -> std::cell::Ref<'_, HostShared> {
        self.state.borrow()
    }
    fn m(&self) -> std::cell::RefMut<'_, HostShared> {
        self.state.borrow_mut()
    }
}

fn bools(v: bool) -> BOOL {
    BOOL::from(v)
}

impl ITextHost_Impl for HostBox {
    fn TxGetDC(&self) -> HDC {
        let hwnd = self.s().hwnd;
        unsafe { GetDC(Some(hwnd)) }
    }
    fn TxReleaseDC(&self, hdc: HDC) -> i32 {
        let hwnd = self.s().hwnd;
        unsafe { ReleaseDC(Some(hwnd), hdc) }
    }
    fn TxShowScrollBar(&self, _fnbar: i32, _fshow: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxEnableScrollBar(&self, _fusbflags: SCROLLBAR_CONSTANTS, _fuarrowflags: i32) -> BOOL {
        BOOL(0)
    }
    fn TxSetScrollRange(&self, _fnbar: i32, _nminpos: i32, _nmaxpos: i32, _fredraw: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxSetScrollPos(&self, _fnbar: i32, _npos: i32, _fredraw: BOOL) -> BOOL {
        BOOL(0)
    }
    fn TxInvalidateRect(&self, _prc: *mut RECT, _fmode: BOOL) {
        self.m().events.push(HostEvent::Invalidate);
    }
    fn TxViewChange(&self, _fupdate: BOOL) {
        self.m().events.push(HostEvent::Invalidate);
    }
    fn TxCreateCaret(&self, _hbmp: HBITMAP, xwidth: i32, yheight: i32) -> BOOL {
        let mut s = self.m();
        s.caret_size = SIZE {
            cx: xwidth,
            cy: yheight,
        };
        s.caret_created = true;
        s.events.push(HostEvent::Caret);
        BOOL(1)
    }
    fn TxShowCaret(&self, fshow: BOOL) -> BOOL {
        let mut s = self.m();
        s.caret_shown = fshow.as_bool();
        s.events.push(HostEvent::Caret);
        s.events.push(HostEvent::Invalidate);
        BOOL(1)
    }
    fn TxSetCaretPos(&self, x: i32, y: i32) -> BOOL {
        let mut s = self.m();
        s.caret_pos = POINT { x, y };
        s.events.push(HostEvent::Caret);
        s.events.push(HostEvent::Invalidate);
        BOOL(1)
    }
    fn TxSetTimer(&self, idtimer: u32, utimeout: u32) -> BOOL {
        let mut s = self.m();
        let hwnd = s.hwnd;
        s.timers.insert(idtimer);
        s.events.push(HostEvent::SetTimer(idtimer, utimeout));
        if hwnd.is_invalid() {
            return BOOL(1);
        }
        unsafe {
            SetTimer(
                Some(hwnd),
                EDIT_TIMER_BASE + idtimer as usize,
                utimeout,
                None,
            ) != 0
        }
        .into()
    }
    fn TxKillTimer(&self, idtimer: u32) {
        let mut s = self.m();
        let hwnd = s.hwnd;
        s.timers.remove(&idtimer);
        s.events.push(HostEvent::KillTimer(idtimer));
        if !hwnd.is_invalid() {
            unsafe {
                let _ = KillTimer(Some(hwnd), EDIT_TIMER_BASE + idtimer as usize);
            }
        }
    }
    fn TxScrollWindowEx(
        &self,
        _dx: i32,
        _dy: i32,
        _lprcscroll: *mut RECT,
        _lprcclip: *mut RECT,
        _hrgnupdate: HRGN,
        _lprcupdate: *mut RECT,
        _fuscroll: SCROLL_WINDOW_FLAGS,
    ) {
        // we repaint the whole frame anyway
        self.m().events.push(HostEvent::Invalidate);
    }
    fn TxSetCapture(&self, fcapture: BOOL) {
        let mut s = self.m();
        s.events.push(HostEvent::Capture(fcapture.as_bool()));
    }
    fn TxSetFocus(&self) {
        let hwnd = self.s().hwnd;
        if !hwnd.is_invalid() {
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
        }
    }
    fn TxSetCursor(&self, hcur: HCURSOR, _ftext: BOOL) {
        unsafe {
            let _ = SetCursor(Some(hcur));
        }
    }
    fn TxScreenToClient(&self, lppt: *mut POINT) -> BOOL {
        let s = self.s();
        let scale = s.scale;
        let hwnd = s.hwnd;
        bools(unsafe {
            let ok = ScreenToClient(hwnd, lppt).as_bool();
            if ok {
                // client units are DIP; ScreenToClient yields device px
                (*lppt).x = ((*lppt).x as f32 / scale).round() as i32;
                (*lppt).y = ((*lppt).y as f32 / scale).round() as i32;
            }
            ok
        })
    }
    fn TxClientToScreen(&self, lppt: *mut POINT) -> BOOL {
        let s = self.s();
        let scale = s.scale;
        let hwnd = s.hwnd;
        bools(unsafe {
            (*lppt).x = ((*lppt).x as f32 * scale).round() as i32;
            (*lppt).y = ((*lppt).y as f32 * scale).round() as i32;
            ClientToScreen(hwnd, lppt).as_bool()
        })
    }
    fn TxActivate(&self, _ploldstate: *mut i32) -> Result<()> {
        Ok(())
    }
    fn TxDeactivate(&self, _lnewstate: i32) -> Result<()> {
        Ok(())
    }
    fn TxGetClientRect(&self, prc: *mut RECT) -> Result<()> {
        unsafe {
            *prc = self.s().client;
        }
        Ok(())
    }
    fn TxGetViewInset(&self, prc: *mut RECT) -> Result<()> {
        unsafe {
            *prc = RECT::default();
        }
        Ok(())
    }
    fn TxGetCharFormat(&self, ppcf: *const *const CHARFORMATW) -> Result<()> {
        unsafe {
            *(ppcf as *mut *const CHARFORMATW) = &*self.s().cf;
        }
        Ok(())
    }
    fn TxGetParaFormat(&self, pppf: *const *const PARAFORMAT) -> Result<()> {
        unsafe {
            *(pppf as *mut *const PARAFORMAT) = &*self.s().pf;
        }
        Ok(())
    }
    fn TxGetSysColor(&self, nindex: SYS_COLOR_INDEX) -> COLORREF {
        let s = self.s();
        match nindex {
            COLOR_WINDOWTEXT => s.fg,
            COLOR_HIGHLIGHT => s.sel_bg,
            COLOR_HIGHLIGHTTEXT => s.sel_fg,
            COLOR_GRAYTEXT => s.fg,
            COLOR_WINDOW => COLORREF(0), // unused: we draw TXTBACK_TRANSPARENT
            _ => unsafe { COLORREF(GetSysColor(nindex)) },
        }
    }
    fn TxGetBackStyle(&self, pstyle: *mut TXTBACKSTYLE) -> Result<()> {
        unsafe {
            *pstyle = TXTBACK_TRANSPARENT;
        }
        Ok(())
    }
    fn TxGetMaxLength(&self, plength: *mut u32) -> Result<()> {
        unsafe {
            *plength = 4000;
        }
        Ok(())
    }
    fn TxGetScrollBars(&self, pdwscrollbar: *mut u32) -> Result<()> {
        unsafe {
            *pdwscrollbar = 0;
        }
        Ok(())
    }
    fn TxGetPasswordChar(&self) -> Result<i8> {
        Ok(0)
    }
    fn TxGetAcceleratorPos(&self, pcp: *mut i32) -> Result<()> {
        unsafe {
            *pcp = -1;
        }
        Ok(())
    }
    fn TxGetExtent(&self, lpextent: *mut SIZE) -> Result<()> {
        let s = self.s();
        // view extent in himetric; client units are DIP -> always 96 dpi,
        // scale-independent, so the format space never latches a stale dpi
        let hm_per_dip = 2540.0 / 96.0;
        unsafe {
            *lpextent = SIZE {
                cx: ((s.client.right - s.client.left) as f32 * hm_per_dip).round() as i32,
                cy: ((s.client.bottom - s.client.top) as f32 * hm_per_dip).round() as i32,
            };
        }
        Ok(())
    }
    fn OnTxCharFormatChange(&self, pcf: *const CHARFORMATW) -> Result<()> {
        unsafe {
            *self.m().cf = *pcf;
        }
        Ok(())
    }
    fn OnTxParaFormatChange(&self, pppf: *const PARAFORMAT) -> Result<()> {
        unsafe {
            *self.m().pf = *pppf;
        }
        Ok(())
    }
    fn TxGetPropertyBits(&self, dwmask: u32, pdwbits: *mut u32) -> Result<()> {
        unsafe {
            *pdwbits = self.s().bits & dwmask;
        }
        Ok(())
    }
    fn TxNotify(&self, inotify: u32, pv: *mut c_void) -> Result<()> {
        if inotify == EN_CHANGE_CODE {
            let mut s = self.m();
            s.dirty = true;
            s.events.push(HostEvent::Change);
        } else if inotify == EN_REQUESTRESIZE_CODE && !pv.is_null() {
            // REQRESIZE { NMHDR nmhdr; RECT rc; } — client-px wanted size
            #[repr(C)]
            struct ReqResize {
                _nm: [usize; 3],
                rc: RECT,
            }
            let rr = unsafe { std::ptr::read_unaligned(pv as *const ReqResize) };
            let mut s = self.m();
            s.natural_px = windows::Win32::Foundation::SIZE {
                cx: rr.rc.right - rr.rc.left,
                cy: rr.rc.bottom - rr.rc.top,
            };
        }
        Ok(())
    }
    fn TxImmGetContext(&self) -> HIMC {
        let hwnd = self.s().hwnd;
        unsafe { ImmGetContext(hwnd) }
    }
    fn TxImmReleaseContext(&self, himc: HIMC) {
        let hwnd = self.s().hwnd;
        unsafe {
            let _ = ImmReleaseContext(hwnd, himc);
        }
    }
    fn TxGetSelectionBarWidth(&self, lselbarwidth: *mut i32) -> Result<()> {
        unsafe {
            *lselbarwidth = 0;
        }
        Ok(())
    }
}

impl ITextHost2_Impl for HostBox {
    fn TxIsDoubleClickPending(&self) -> BOOL {
        BOOL(0)
    }
    fn TxGetWindow(&self, phwnd: *mut HWND) -> Result<()> {
        unsafe {
            *phwnd = self.s().hwnd;
        }
        Ok(())
    }
    fn TxSetForegroundWindow(&self) -> Result<()> {
        Ok(())
    }
    fn TxGetPalette(&self) -> HPALETTE {
        HPALETTE::default()
    }
    fn TxGetEastAsianFlags(&self, pflags: *mut i32) -> Result<()> {
        unsafe {
            *pflags = 0;
        }
        Ok(())
    }
    fn TxSetCursor2(&self, hcur: HCURSOR, _btext: BOOL) -> HCURSOR {
        unsafe {
            let _ = SetCursor(Some(hcur));
            hcur
        }
    }
    fn TxFreeTextServicesNotification(&self) {}
    fn TxGetEditStyle(&self, _dwitem: u32, pdwdata: *mut u32) -> Result<()> {
        unsafe {
            *pdwdata = 0;
        }
        Ok(())
    }
    fn TxGetWindowStyles(&self, pdwstyle: *mut u32, pdwexstyle: *mut u32) -> Result<()> {
        unsafe {
            *pdwstyle = WS_CHILD.0 | WS_VISIBLE.0 | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32;
            *pdwexstyle = 0;
        }
        Ok(())
    }
    fn TxShowDropCaret(&self, _fshow: BOOL, _hdc: HDC, _prc: *mut RECT) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn TxDestroyCaret(&self) -> Result<()> {
        Ok(())
    }
    fn TxGetHorzExtent(&self, plhorzextent: *mut i32) -> Result<()> {
        unsafe {
            *plhorzextent = 0;
        }
        Ok(())
    }
}

/// DIP -> HIMETRIC for the text-services API surface.
pub fn dip_to_himetric(dip: f32) -> i32 {
    (dip * 2540.0 / 96.0).round() as i32
}

/// Configuration for one editor instance.
#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub face: String,
    /// Font size in twips (1/20 pt). 14 DIP body text = 210 twips.
    pub size_twips: i32,
    pub fg: [f32; 4],
    pub sel_bg: [f32; 4],
    pub sel_fg: [f32; 4],
    pub read_only: bool,
}

/// `CreateTextServices` + the plain-text/D2D init sequence. Shared between
/// `Editor::new` and the DPI-recreate path.
fn create_tx(lib: &Msftedit, host: *mut c_void, client_dip: &RECT) -> Result<ITextServices2> {
    unsafe {
        let mut punk: *mut c_void = std::ptr::null_mut();
        (lib.create_text_services)(std::ptr::null_mut(), host, &mut punk).ok()?;
        let unk = IUnknown::from_raw(punk);
        let mut tx2_raw: *mut c_void = std::ptr::null_mut();
        (windows::core::Interface::vtable(&unk).QueryInterface)(
            unk.as_raw(),
            &lib.iid_text_services2 as *const GUID,
            &mut tx2_raw,
        )
        .ok()?;
        let tx2 = ITextServices2::from_raw(tx2_raw);
        let mut res = LRESULT(0);
        let _ = tx2.TxSendMessage(
            EM_SETTEXTMODE,
            WPARAM(TM_PLAINTEXT.0 as usize),
            LPARAM(0),
            &mut res,
        );
        let _ = tx2.TxSendMessage(
            EM_SETEVENTMASK,
            WPARAM(0),
            LPARAM(
                (ENM_CHANGE
                    | ENM_REQUESTRESIZE
                    | ENM_KEYEVENTS
                    | ENM_MOUSEEVENTS
                    | ENM_SELCHANGE
                    | ENM_SCROLL) as isize,
            ),
            &mut res,
        );
        let _ = tx2.TxSendMessage(EM_AUTOURLDETECT, WPARAM(0), LPARAM(0), &mut res);
        let _ = tx2.TxSendMessage(EM_LIMITTEXT, WPARAM(4000), LPARAM(0), &mut res);
        let _ = tx2.OnTxInPlaceActivate(client_dip as *const RECT as *mut RECT);
        let _ = tx2.OnTxUIActivate();
        Ok(tx2)
    }
}

/// A windowless RichEdit instance (plain text, multiline, D2D render path).
pub struct Editor {
    tx: Option<ITextServices2>,
    host: *mut HostBox,
    pub hwnd: HWND,
    /// Lazily-built UIA providers (rich text child + our fragment root).
    uia: std::cell::RefCell<
        Option<(
            windows::Win32::UI::Accessibility::IRawElementProviderSimple,
            windows::Win32::UI::Accessibility::IRawElementProviderFragmentRoot,
        )>,
    >,
}

impl Editor {
    pub fn new(hwnd: HWND, client_dip: RECT, scale: f32, cfg: &EditorConfig) -> Result<Editor> {
        let lib = Msftedit::load()?;
        let mut face = [0u16; 32];
        for (i, c) in cfg.face.encode_utf16().take(31).enumerate() {
            face[i] = c;
        }
        let shared = HostShared {
            hwnd,
            client: client_dip,
            scale,
            bits: TXTBIT_MULTILINE
                | TXTBIT_WORDWRAP
                | TXTBIT_AUTOWORDSEL
                | TXTBIT_DISABLEDRAG
                | TXTBIT_D2DDWRITE
                | TXTBIT_D2DPIXELSNAPPED,
            natural_px: SIZE::default(),
            cf: Box::new(CHARFORMATW {
                cbSize: std::mem::size_of::<CHARFORMATW>() as u32,
                dwMask: CFM_ALL,
                // CFE_AUTOCOLOR: text colour resolves through TxGetSysColor
                // (COLOR_WINDOWTEXT) at draw time, so a theme switch recolors
                // existing runs without rewriting character formats.
                dwEffects: CFE_EFFECTS(CFE_AUTOCOLOR.0),
                yHeight: cfg.size_twips,
                yOffset: 0,
                crTextColor: colorref(cfg.fg),
                bCharSet: DEFAULT_CHARSET,
                bPitchAndFamily: 0,
                szFaceName: face,
            }),
            pf: Box::new(PARAFORMAT {
                cbSize: std::mem::size_of::<PARAFORMAT>() as u32,
                ..Default::default()
            }),
            fg: colorref(cfg.fg),
            sel_bg: colorref(cfg.sel_bg),
            sel_fg: colorref(cfg.sel_fg),
            caret_pos: POINT::default(),
            caret_size: SIZE::default(),
            caret_shown: false,
            caret_created: false,
            timers: BTreeSet::new(),
            events: Vec::new(),
            read_only: cfg.read_only,
            dim: None,
            dirty: false,
        };
        let host = Box::into_raw(Box::new(HostBox {
            host: unsafe { &*(&VT_HOST2 as *const ITextHost2_Vtbl as *const ITextHost_Vtbl) },
            host2: &VT_HOST2,
            refs: Cell::new(1),
            state: RefCell::new(shared),
        }));
        let tx2 = create_tx(lib, host as *mut c_void, &client_dip)?;
        Ok(Editor {
            tx: Some(tx2),
            host,
            hwnd,
            uia: std::cell::RefCell::new(None),
        })
    }

    fn tx(&self) -> &ITextServices2 {
        self.tx.as_ref().unwrap()
    }

    /// Raw `QueryInterface` on the text services object for an arbitrary IID
    /// — `IRicheditWindowlessAccessibility` has a zero IID in windows-rs, so
    /// the real one comes from msftedit's data exports.
    pub fn query_iid(&self, iid: &GUID) -> Result<*mut c_void> {
        unsafe {
            let mut p: *mut c_void = std::ptr::null_mut();
            let vt = windows::core::Interface::vtable(self.tx());
            (vt.base__.base__.QueryInterface)(self.tx().as_raw(), iid as *const GUID, &mut p)
                .ok()?;
            Ok(p)
        }
    }

    /// Lazily builds and returns the UIA fragment root for this editor
    /// (see [`crate::uia`]); `None` when the windowless accessibility
    /// interface isn't available.
    pub fn uia_root(
        &self,
    ) -> Option<windows::Win32::UI::Accessibility::IRawElementProviderFragmentRoot> {
        if let Some((_, root)) = self.uia.borrow().as_ref() {
            return Some(root.clone());
        }
        match crate::uia::build_providers(self, self.hwnd) {
            Ok((child, root)) => {
                *self.uia.borrow_mut() = Some((child, root.clone()));
                Some(root)
            }
            Err(_) => None,
        }
    }

    fn shared(&self) -> std::cell::Ref<'_, HostShared> {
        unsafe { (*self.host).state.borrow() }
    }
    fn shared_mut(&self) -> std::cell::RefMut<'_, HostShared> {
        unsafe { (*self.host).state.borrow_mut() }
    }

    /// Drains host events accumulated during the last call.
    pub fn drain_events(&self) -> Vec<HostEvent> {
        std::mem::take(&mut self.shared_mut().events)
    }

    /// Forwards a window message into the text service; returns LRESULT.
    pub fn send(&self, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe {
            let mut res = LRESULT(0);
            let _ = self
                .tx()
                .TxSendMessage(msg, WPARAM(wparam), LPARAM(lparam), &mut res);
            res.0
        }
    }

    pub fn set_text(&self, text: &str) -> Result<()> {
        unsafe {
            let h = HSTRING::from(text);
            self.tx().TxSetText(PCWSTR(h.as_ptr()))
        }
    }

    pub fn text(&self) -> String {
        unsafe {
            let mut b = BSTR::default();
            if self.tx().TxGetText(&mut b).is_ok() {
                b.to_string()
            } else {
                String::new()
            }
        }
    }

    /// Selection as (anchor, active) char positions.
    pub fn selection(&self) -> (i32, i32) {
        let mut cr = CHARRANGE::default();
        self.send(EM_EXGETSEL, 0, &mut cr as *mut _ as isize);
        (cr.cpMin, cr.cpMax)
    }

    pub fn set_selection(&self, min: i32, max: i32) {
        let cr = CHARRANGE {
            cpMin: min,
            cpMax: max,
        };
        self.send(EM_EXSETSEL, 0, &cr as *const _ as isize);
    }

    /// True when the text content is empty. Uses EM_GETTEXTLENGTHEX rather
    /// than TxGetText: a transient TxGetText failure must never read as
    /// "empty" (it would suppress Enter submission with text present).
    pub fn is_empty(&self) -> bool {
        let gtl = GETTEXTLENGTHEX {
            flags: GTL_PRECISE | GTL_NUMCHARS,
            codepage: 1200,
        };
        self.send(EM_GETTEXTLENGTHEX, &gtl as *const _ as usize, 0) == 0
    }

    /// Raw REQRESIZE size in client-rect units (device px) — diagnostic.
    pub fn natural_size_raw(&self) -> SIZE {
        self.send(EM_REQUESTRESIZE, 0, 0);
        self.shared_mut().natural_px
    }

    /// Natural content size in DIP. msftedit formats text in the DIP space
    /// of `TxGetClientRect`, so the client *width* must be current (wrap)
    /// before measuring; REQRESIZE replies in the same DIP space.
    pub fn natural_size(&self, width_dip: f32) -> Result<(f32, f32)> {
        let w = width_dip.round() as i32;
        {
            let mut st = self.shared_mut();
            if st.client.right - st.client.left != w {
                st.client = RECT {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: st.client.bottom.max(4000),
                };
            }
        }
        // EM_REQUESTRESIZE -> synchronous EN_REQUESTRESIZE -> natural_px
        self.send(EM_REQUESTRESIZE, 0, 0);
        let px = self.shared_mut().natural_px;
        Ok((px.cx as f32, px.cy as f32))
    }

    /// Draws the editor content (incl. selection, caret drawn by painter) into
    /// the current D2D target. `bounds_dip` is the editor rect in DIP.
    /// `lprcBounds` for TxDrawD2D is in the render target's logical units
    /// (DIPs when the target's DPI is set — i.e. the painter's DIP space).
    pub fn draw_d2d(&self, rt: &ID2D1RenderTarget, bounds_dip: (f32, f32, f32, f32)) -> Result<()> {
        let (x, y, r, b) = bounds_dip;
        let rc = RECTL {
            left: x.round() as i32,
            top: y.round() as i32,
            right: r.round() as i32,
            bottom: b.round() as i32,
        };
        unsafe {
            self.tx().TxDrawD2D(
                rt,
                &rc as *const RECTL as *mut RECTL,
                std::ptr::null_mut(),
                0,
            )
        }
    }

    /// Caret rect in client pixels (for the painter to draw) when created+shown.
    pub fn caret(&self) -> Option<(POINT, SIZE)> {
        let s = self.shared();
        (s.caret_created && s.caret_shown).then(|| (s.caret_pos, s.caret_size))
    }

    /// Raw caret state for "frozen solid" blink-timeout drawing.
    pub fn caret_info(&self) -> (bool, POINT, SIZE) {
        let s = self.shared();
        (s.caret_created, s.caret_pos, s.caret_size)
    }

    /// Set the editor client rect in DIPs.
    pub fn set_client_rect(&self, dip: RECT) {
        self.shared_mut().client = dip;
    }

    /// Owning window (IME context, timers, capture). Called when the editor is
    /// created before the window exists.
    pub fn set_hwnd(&self, hwnd: HWND) {
        self.shared_mut().hwnd = hwnd;
    }

    /// Keeps the caret drawn solid (used after the blink timeout freezes it).
    pub fn force_caret_shown(&self) {
        let mut s = self.shared_mut();
        if s.caret_created {
            s.caret_shown = true;
            s.events.push(HostEvent::Caret);
        }
    }

    /// Host-driven blink toggle: windowless richedit does not blink the caret
    /// itself, so the app's caret timer flips `caret_shown` (no-op until the
    /// caret is created).
    pub fn toggle_caret(&self) {
        let mut s = self.shared_mut();
        if s.caret_created {
            s.caret_shown = !s.caret_shown;
            s.events.push(HostEvent::Caret);
        }
    }

    /// DPI scale of the render target changed. The editor's coordinate space
    /// is DIP (scale-independent), so nothing needs to be recreated or
    /// re-measured — scale only enters when converting client units to
    /// screen pixels for `TxScreenToClient`/`TxClientToScreen`.
    pub fn set_scale(&mut self, scale: f32) -> Result<()> {
        self.shared_mut().scale = scale;
        Ok(())
    }

    pub fn set_read_only(&self, ro: bool) {
        self.shared_mut().read_only = ro;
    }

    /// Recolors the editor. Existing text picks up the new fg immediately via
    /// `CFE_AUTOCOLOR` + `TxGetSysColor` at draw time — deliberately NOT
    /// `OnTxPropertyBitsChange(TXTBIT_CHARFORMATCHANGE)`: that notification
    /// makes msftedit re-derive its D2D unit mapping and corrupts the text
    /// scale (2x-size glyphs, stale wrap width) at scale != 1.
    pub fn set_colors(&self, fg: [f32; 4], sel_bg: [f32; 4], sel_fg: [f32; 4]) {
        let mut s = self.shared_mut();
        s.fg = colorref(fg);
        s.sel_bg = colorref(sel_bg);
        s.sel_fg = colorref(sel_fg);
        s.cf.crTextColor = colorref(fg);
        s.events.push(HostEvent::Invalidate);
    }

    /// Dims (or un-dims) all text: `Some(color)` applies a fixed muted tone,
    /// `None` restores `CFE_AUTOCOLOR` so runs follow the theme syscolor.
    pub fn dim_text(&self, color: Option<[f32; 4]>) {
        let cf = CHARFORMATW {
            cbSize: std::mem::size_of::<CHARFORMATW>() as u32,
            dwMask: CFM_MASK(CFM_EFFECTS.0 | CFM_COLOR.0),
            dwEffects: CFE_EFFECTS(match color {
                Some(_) => 0, // fixed color -> autocolor off
                None => CFE_AUTOCOLOR.0,
            }),
            crTextColor: color.map(colorref).unwrap_or(COLORREF(0)),
            ..Default::default()
        };
        self.send(EM_SETCHARFORMAT, SCF_ALL as usize, &cf as *const _ as isize);
        let mut s = self.shared_mut();
        s.dim = color;
        s.events.push(HostEvent::Invalidate);
    }

    /// Editor client rect in DIPs.
    pub fn client_rect(&self) -> RECT {
        self.shared().client
    }

    /// Whether an editor-owned timer is installed (richedit blink timer).
    pub fn has_timers(&self) -> bool {
        !self.shared().timers.is_empty()
    }

    /// RichEdit-requested timer ids currently armed (diagnostics).
    pub fn timer_ids(&self) -> Vec<u32> {
        self.shared().timers.iter().copied().collect()
    }

    /// Kill all richedit-requested host timers (caret timeout path).
    pub fn kill_timers(&self) {
        let ids: Vec<u32> = self.shared().timers.iter().copied().collect();
        for id in ids {
            let hwnd = self.shared().hwnd;
            self.shared_mut().timers.remove(&id);
            if !hwnd.is_invalid() {
                unsafe {
                    let _ = KillTimer(Some(hwnd), EDIT_TIMER_BASE + id as usize);
                }
            }
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        // release the text service first (it may release host references),
        // then our own reference to the host object
        drop(self.tx.take());
        unsafe {
            HostBox::Release(self.host);
        }
    }
}
