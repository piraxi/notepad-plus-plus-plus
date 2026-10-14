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
            WM_CAPTURECHANGED | WM_CANCELMODE => {
                RemovePropW(hwnd, windows_sys::core::w!("DragTab"));
                RemovePropW(hwnd, windows_sys::core::w!("DragOrigin"));
                if msg == WM_CANCELMODE && GetCapture() == hwnd {
                    ReleaseCapture();
                }
            }
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
                    let close_visible = GetPropW(hwnd, windows_sys::core::w!("HideTabClose"))
                        .is_null()
                        && (GetPropW(hwnd, windows_sys::core::w!("HideInactiveTabClose"))
                            .is_null()
                            || SendMessageW(hwnd, TCM_GETCURSEL, 0, 0) == index as isize);
                    if close_visible && lp as i16 as i32 >= rect.right - 22 {
                        enqueue(Event::CloseTab(index));
                        return 0;
                    }
                    if !GetPropW(hwnd, windows_sys::core::w!("LockTabDrag")).is_null() {
                        return DefSubclassProc(hwnd, msg, wp, lp);
                    }
                    SetPropW(
                        hwnd,
                        windows_sys::core::w!("DragTab"),
                        (index + 1) as HANDLE,
                    );
                    SetPropW(
                        hwnd,
                        windows_sys::core::w!("DragOrigin"),
                        ((lp as u32 as usize) + 1) as HANDLE,
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
                    && ((lp as i16 as i32 - ((origin - 1) as u16 as i16 as i32)).abs()
                        > GetSystemMetrics(SM_CXDRAG)
                        || ((lp >> 16) as i16 as i32 - (((origin - 1) >> 16) as u16 as i16 as i32))
                            .abs()
                            > GetSystemMetrics(SM_CYDRAG))
                {
                    if let Some(to) = hit(hwnd, lp) {
                        enqueue(Event::MoveTab(from - 1, to));
                    }
                }
            }
            WM_LBUTTONDBLCLK => {
                RemovePropW(hwnd, windows_sys::core::w!("DragTab"));
                RemovePropW(hwnd, windows_sys::core::w!("DragOrigin"));
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                if let Some(index) = hit(hwnd, lp) {
                    enqueue(Event::DoubleClickTab(index));
                } else {
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

pub fn compact_label(name: &str, limit: u32) -> String {
    let name_utf16: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    if limit == 0 || name_utf16.len() <= limit as usize + 1 {
        return name.to_owned();
    }
    let mut output = vec![0u16; limit as usize + 1];
    unsafe {
        if windows_sys::Win32::UI::Shell::PathCompactPathExW(
            output.as_mut_ptr(),
            name_utf16.as_ptr(),
            output.len() as u32,
            0,
        ) == 0
        {
            return name.to_owned();
        }
    }
    let length = output.iter().position(|&c| c == 0).unwrap_or(output.len());
    String::from_utf16(&output[..length]).unwrap_or_else(|_| name.to_owned())
}

pub fn close_buttons(hwnd: HWND, show: bool, inactive: bool) {
    unsafe {
        for (name, hide) in [
            (windows_sys::core::w!("HideTabClose"), !show),
            (windows_sys::core::w!("HideInactiveTabClose"), !inactive),
        ] {
            if hide {
                SetPropW(hwnd, name, 1usize as HANDLE);
            } else {
                RemovePropW(hwnd, name);
            }
        }
    }
}

pub fn lock_drag(hwnd: HWND, locked: bool) {
    unsafe {
        if locked {
            SetPropW(hwnd, windows_sys::core::w!("LockTabDrag"), 1usize as HANDLE);
            RemovePropW(hwnd, windows_sys::core::w!("DragTab"));
            RemovePropW(hwnd, windows_sys::core::w!("DragOrigin"));
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
        } else {
            RemovePropW(hwnd, windows_sys::core::w!("LockTabDrag"));
        }
    }
}

pub fn install(hwnd: HWND) -> bool {
    unsafe { SetWindowSubclass(hwnd, Some(procedure), 2, 0) != 0 }
}
