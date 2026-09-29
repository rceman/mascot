//! UIA bridge for the windowless RichEdit composer (bounded Phase-A attempt).
//!
//! `TextServ.h` contract: the host implements
//! `IRawElementProviderWindowlessSite` (answers the editor provider's
//! `GetAdjacentFragment(Parent)` with the window's root fragment), QIs
//! `ITextServices2` for `IRicheditWindowlessAccessibility` (the IID lives in
//! msftedit's data exports like the other text-service IIDs), calls
//! `CreateProvider(site)` for the document/edit provider, and answers
//! `WM_GETOBJECT(UiaRootObjectId)` with `UiaReturnRawElementProvider`
//! carrying a minimal fragment root whose single child is that provider.
//!
//! Absent navigation directions return `E_NOTIMPL` (the windows-rs
//! `Result<Interface>` return convention cannot express a NULL out-param
//! with S_OK; UIA clients treat the failure as "no element").

use std::cell::RefCell;

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

/// The site the RichEdit windowless provider calls back through.
#[implement(IRawElementProviderWindowlessSite)]
struct WindowlessSite {
    root: RefCell<Option<IRawElementProviderFragment>>,
}

impl IRawElementProviderWindowlessSite_Impl for WindowlessSite_Impl {
    fn GetAdjacentFragment(
        &self,
        direction: NavigateDirection,
    ) -> Result<IRawElementProviderFragment> {
        if direction == NavigateDirection_Parent
            && let Some(root) = self.root.borrow().as_ref()
        {
            return Ok(root.clone());
        }
        Err(E_NOTIMPL.into())
    }

    fn GetRuntimeIdPrefix(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
}

/// Minimal fragment root: HWND-backed window provider, one child (the
/// windowless RichEdit provider).
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct RootFragment {
    hwnd: HWND,
    child: IRawElementProviderSimple,
    me: RefCell<Option<IRawElementProviderFragmentRoot>>,
}

impl IRawElementProviderSimple_Impl for RootFragment_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, _patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if propertyid == UIA_NamePropertyId {
            let b = BSTR::from("Mascot composer");
            return Ok(VARIANT::from(b));
        }
        Ok(VARIANT::default())
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(self.hwnd) }
    }
}

impl IRawElementProviderFragment_Impl for RootFragment_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        if direction == NavigateDirection_FirstChild {
            return self.child.cast::<IRawElementProviderFragment>();
        }
        Err(E_NOTIMPL.into())
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        unsafe {
            let mut rc = RECT::default();
            let _ = GetWindowRect(self.hwnd, &mut rc);
            Ok(UiaRect {
                left: rc.left as f64,
                top: rc.top as f64,
                width: (rc.right - rc.left) as f64,
                height: (rc.bottom - rc.top) as f64,
            })
        }
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        unsafe { self.child.cast::<IRawElementProviderFragment>()?.SetFocus() }
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.me.borrow().clone().ok_or_else(|| E_NOTIMPL.into())
    }
}

impl IRawElementProviderFragmentRoot_Impl for RootFragment_Impl {
    fn ElementProviderFromPoint(&self, _x: f64, _y: f64) -> Result<IRawElementProviderFragment> {
        self.child.cast::<IRawElementProviderFragment>()
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        self.child.cast::<IRawElementProviderFragment>()
    }
}

/// Builds the (site, provider, root) triple for `editor`'s text services —
/// QI'ing for the exported `IID_IRicheditWindowlessAccessibility`. Returns
/// the provider (child) and the fragment root to answer `WM_GETOBJECT` with.
/// `site_obj`/`root_obj` are kept alive by their interface references.
pub fn build_providers(
    editor: &crate::edit::Editor,
    hwnd: HWND,
) -> Result<(IRawElementProviderSimple, IRawElementProviderFragmentRoot)> {
    let iid = crate::edit::iid_windowless_accessibility()?;
    let raw = editor.query_iid(&iid)?;
    let acc: IRicheditWindowlessAccessibility = unsafe { Interface::from_raw(raw) };
    let site_co = windows_core::ComObject::new(WindowlessSite {
        root: RefCell::new(None),
    });
    let site: IRawElementProviderWindowlessSite = site_co.to_interface();
    let child = unsafe { acc.CreateProvider(&site)? };

    let root_co = windows_core::ComObject::new(RootFragment {
        hwnd,
        child: child.clone(),
        me: RefCell::new(None),
    });
    let root: IRawElementProviderFragmentRoot = root_co.to_interface();
    // the fragment root returns itself from FragmentRoot, and the site
    // answers the child's GetAdjacentFragment(Parent)
    *root_co.get().me.borrow_mut() = Some(root.clone());
    let root_frag: IRawElementProviderFragment = root_co.to_interface();
    *site_co.get().root.borrow_mut() = Some(root_frag);
    Ok((child, root))
}
