use crate::platform::Ui;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows_sys::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows_sys::Win32::UI::Input::Ime::{
    CPS_CANCEL, ImmGetContext, ImmNotifyIME, ImmReleaseContext, NI_COMPOSITIONSTR,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_RETURN};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const EM_GETTEXTEX: u32 = WM_USER + 94;
pub const EM_GETTEXTLENGTHEX: u32 = WM_USER + 95;
pub const EM_SETTEXTEX: u32 = WM_USER + 97;
pub const EM_SETTEXTMODE: u32 = WM_USER + 89;
pub const EM_SETLIMITTEXT: u32 = 0x00C5;
pub const EM_SETUNDOLIMIT: u32 = WM_USER + 82;
pub const EM_SETTYPOGRAPHYOPTIONS: u32 = WM_USER + 202;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_REPLACESEL: u32 = 0x00C2;
pub const EM_SCROLLCARET: u32 = 0x00B7;
pub const EM_EXGETSEL: u32 = WM_USER + 52;
pub const EM_EXSETSEL: u32 = WM_USER + 55;

pub const TM_PLAINTEXT: usize = 1;
pub const TM_MULTILEVELUNDO: usize = 8;
pub const TM_MULTICODEPAGE: usize = 32;
pub const TO_ADVANCEDTYPOGRAPHY: usize = 0x0001;

pub const ES_MULTILINE: u32 = 0x0004;
pub const ES_WANTRETURN: u32 = 0x1000;
pub const ES_READONLY: u32 = 0x0800;
pub const ES_NOHIDESEL: u32 = 0x0100;

#[repr(C)]
struct SetTextEx {
    flags: u32,
    codepage: u32,
}

#[repr(C)]
struct GetTextEx {
    cb: u32,
    flags: u32,
    codepage: u32,
    default_char: *const i8,
    used_default: *mut i32,
}

#[repr(C)]
struct GetTextLengthEx {
    flags: u32,
    codepage: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CharRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Clone)]
pub struct TextSnapshot {
    pub input: String,
    pub response: String,
    pub selection_start: i32,
    pub selection_end: i32,
    pub composing: bool,
}

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn load_richedit() -> Result<HMODULE, String> {
    unsafe {
        if OleInitialize(std::ptr::null_mut()) < 0 {
            return Err("OleInitialize failed".into());
        }
        let module = LoadLibraryExW(
            wide("msftedit.dll").as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            OleUninitialize();
            return Err("msftedit.dll load failed".into());
        }
        Ok(module)
    }
}

pub unsafe fn create_edit(
    parent: HWND,
    instance: windows_sys::Win32::Foundation::HINSTANCE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    read_only: bool,
    char_limit: usize,
    undo_limit: usize,
    font: windows_sys::Win32::Graphics::Gdi::HFONT,
    control_id: usize,
) -> Result<HWND, String> {
    unsafe {
        let mut style =
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_WANTRETURN | ES_NOHIDESEL;
        if read_only {
            style |= ES_READONLY;
        }
        let class = wide("RICHEDIT50W");
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            style,
            x,
            y,
            width,
            height,
            parent,
            control_id as *mut c_void,
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(format!(
                "edit control creation failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        SendMessageW(
            hwnd,
            EM_SETTEXTMODE,
            TM_PLAINTEXT | TM_MULTILEVELUNDO | TM_MULTICODEPAGE,
            0,
        );
        SendMessageW(
            hwnd,
            EM_SETTYPOGRAPHYOPTIONS,
            TO_ADVANCEDTYPOGRAPHY,
            TO_ADVANCEDTYPOGRAPHY as isize,
        );
        SendMessageW(hwnd, EM_SETLIMITTEXT, char_limit, 0);
        SendMessageW(hwnd, EM_SETUNDOLIMIT, undo_limit, 0);
        SendMessageW(hwnd, WM_SETFONT, font as usize, 1);
        Ok(hwnd)
    }
}

pub fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn get_text(hwnd: HWND) -> String {
    unsafe {
        let length_query = GetTextLengthEx {
            flags: 0,
            codepage: 1200,
        };
        let units = SendMessageW(
            hwnd,
            EM_GETTEXTLENGTHEX,
            &length_query as *const _ as usize,
            0,
        );
        let mut buffer = vec![0u16; units as usize + 1];
        let query = GetTextEx {
            cb: (buffer.len() * 2) as u32,
            flags: 0,
            codepage: 1200,
            default_char: std::ptr::null(),
            used_default: std::ptr::null_mut(),
        };
        let copied = SendMessageW(
            hwnd,
            EM_GETTEXTEX,
            &query as *const _ as usize,
            buffer.as_mut_ptr() as isize,
        );
        buffer.truncate(copied.max(0) as usize);
        normalize(&String::from_utf16_lossy(&buffer))
    }
}

pub fn append_text(hwnd: HWND, value: &str) {
    let value = wide(&normalize(value).replace('\n', "\r"));
    unsafe {
        SendMessageW(hwnd, EM_SETSEL, usize::MAX, -1);
        SendMessageW(hwnd, EM_REPLACESEL, 0, value.as_ptr() as isize);
        SendMessageW(hwnd, EM_SCROLLCARET, 0, 0);
    }
}

pub fn set_text(hwnd: HWND, text: &str) {
    let crlf = text.replace('\n', "\r\n");
    let wide_text = wide(&crlf);
    let options = SetTextEx {
        flags: 0,
        codepage: 1200,
    };
    unsafe {
        SendMessageW(
            hwnd,
            EM_SETTEXTEX,
            &options as *const _ as usize,
            wide_text.as_ptr() as isize,
        );
    }
}

pub fn get_selection(hwnd: HWND) -> CharRange {
    let mut range = CharRange { min: 0, max: 0 };
    unsafe {
        SendMessageW(hwnd, EM_EXGETSEL, 0, &mut range as *mut _ as isize);
    }
    range
}

pub fn set_selection(hwnd: HWND, min: i32, max: i32) {
    let range = CharRange { min, max };
    unsafe {
        SendMessageW(hwnd, EM_EXSETSEL, 0, &range as *const _ as isize);
    }
}

pub fn utf16_units(text: &str) -> usize {
    text.encode_utf16().count()
}

pub fn cancel_composition(hwnd: HWND) {
    unsafe {
        let context = ImmGetContext(hwnd);
        if !context.is_null() {
            ImmNotifyIME(context, NI_COMPOSITIONSTR, CPS_CANCEL, 0);
            ImmReleaseContext(hwnd, context);
        }
    }
}

unsafe extern "system" fn control_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    reference: usize,
) -> LRESULT {
    unsafe {
        let ui = &*(reference as *const Ui);
        if message == WM_PAINT {
            ui.count_paint();
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(control_subclass), subclass_id);
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        if subclass_id != 1 {
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        match message {
            WM_IME_STARTCOMPOSITION => {
                ui.on_ime_start(hwnd);
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
            WM_IME_ENDCOMPOSITION => {
                ui.on_ime_end(hwnd);
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
            WM_IME_COMPOSITION => {
                ui.on_ime_update();
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
            WM_KEYDOWN => {
                let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
                if wparam as u16 == VK_RETURN as u16 && ctrl && !ui.composing() {
                    ui.post_submit();
                    return 0;
                }
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, message, wparam, lparam),
        }
    }
}

pub fn subclass_control(hwnd: HWND, ui: *const Ui, input: bool) -> Result<(), String> {
    unsafe {
        let id = if input { 1 } else { 2 };
        if SetWindowSubclass(hwnd, Some(control_subclass), id, ui as usize) == 0 {
            return Err("SetWindowSubclass failed".into());
        }
    }
    Ok(())
}

pub fn instance() -> windows_sys::Win32::Foundation::HINSTANCE {
    unsafe {
        let mut module: HMODULE = std::ptr::null_mut();
        GetModuleHandleExW(0, std::ptr::null(), &mut module);
        module
    }
}
