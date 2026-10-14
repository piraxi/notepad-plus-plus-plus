use crate::app::{Event, enqueue};
use npp_compat::commands::*;
use std::ptr::null;
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::{
    Foundation::*,
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

fn hit(hwnd: HWND, lp: LPARAM) -> Option<usize> {
    let mut hit = TCHITTESTINFO {
        pt: POINT {
            x: lp as i16 as i32,
            y: (lp >> 16) as i16 as i32,
        },
        flags: 0,
    };
    let index = unsafe { SendMessageW(hwnd, TCM_HITTEST, 0, &mut hit as *mut _ as isize) };
    (index >= 0).then_some(index as usize)
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    // SAFETY: This subclass is installed only on the live tab control.
    unsafe {
        match msg {
            WM_MBUTTONUP => {
                if let Some(index) = hit(hwnd, lp) {
                    enqueue(Event::CloseTab(index));
                }
                return 0;
            }
            WM_LBUTTONDOWN => {
                if let Some(index) = hit(hwnd, lp) {
                    let mut rect = RECT::default();
                    SendMessageW(hwnd, TCM_GETITEMRECT, index, &mut rect as *mut _ as isize);
                    if lp as i16 as i32 >= rect.right - 22 {
                        enqueue(Event::CloseTab(index));
                        return 0;
                    }
                    SetPropW(
                        hwnd,
                        windows_sys::core::w!("DragTab"),
                        (index + 1) as HANDLE,
                    );
                    SetPropW(
                        hwnd,
                        windows_sys::core::w!("DragOrigin"),
                        ((lp as u16 as usize) + 1) as HANDLE,
                    );
                    SetCapture(hwnd);
                }
            }
            WM_LBUTTONUP => {
                let from = RemovePropW(hwnd, windows_sys::core::w!("DragTab")) as usize;
                let origin = RemovePropW(hwnd, windows_sys::core::w!("DragOrigin")) as usize;
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                if from != 0
                    && (lp as i16 as i32 - (origin as i32 - 1)).abs() > GetSystemMetrics(SM_CXDRAG)
                {
                    if let Some(to) = hit(hwnd, lp) {
                        enqueue(Event::MoveTab(from - 1, to));
                    }
                }
            }
            WM_LBUTTONDBLCLK => {
                if hit(hwnd, lp).is_none() {
                    enqueue(Event::Command(IDM_FILE_NEW));
                }
                return 0;
            }
            WM_CONTEXTMENU => {
                let mut point = POINT::default();
                GetCursorPos(&mut point);
                let mut local = point;
                ScreenToClient(hwnd, &mut local);
                if let Some(index) =
                    hit(hwnd, ((local.y as isize) << 16) | (local.x as u16 as isize))
                {
                    enqueue(Event::Tab(index));
                }
                let menu = CreatePopupMenu();
                for (id, label) in [
                    (IDM_FILE_SAVE, "Save"),
                    (IDM_FILE_SAVEAS, "Save As…"),
                    (IDM_FILE_CLOSE, "Close"),
                    (IDM_FILE_CLOSEALL_BUT_CURRENT, "Close All but This"),
                    (IDM_FILE_CLOSEALL_TOLEFT, "Close All to the Left"),
                    (IDM_FILE_CLOSEALL_TORIGHT, "Close All to the Right"),
                    (IDM_EDIT_FULLPATHTOCLIP, "Copy Full Path"),
                    (IDM_VIEW_TAB_MOVEBACKWARD, "Move Tab Left"),
                    (IDM_VIEW_TAB_MOVEFORWARD, "Move Tab Right"),
                ] {
                    let text: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    AppendMenuW(menu, MF_STRING, id as usize, text.as_ptr());
                }
                let command = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    0,
                    hwnd,
                    null(),
                );
                DestroyMenu(menu);
                if command != 0 {
                    enqueue(Event::Command(command as u32));
                }
                return 0;
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}

pub fn install(hwnd: HWND) -> bool {
    unsafe { SetWindowSubclass(hwnd, Some(procedure), 2, 0) != 0 }
}
