//! Minimal native test DLL with the layouts from PluginInterface.h.
use std::{ffi::c_void, sync::atomic::{AtomicIsize, AtomicUsize, Ordering}};

#[repr(C)]
pub struct NppData { npp: *mut c_void, main: *mut c_void, second: *mut c_void }
#[repr(C)]
pub struct FuncItem {
    name: [u16; 64],
    function: Option<unsafe extern "C" fn()>,
    command_id: i32,
    checked: bool,
    shortcut: *mut c_void,
}
static CALLS: AtomicIsize = AtomicIsize::new(0);
static HOST: AtomicUsize = AtomicUsize::new(0);
static LAST_NOTIFICATION: AtomicIsize = AtomicIsize::new(0);
static HOST_ALIVE: AtomicIsize = AtomicIsize::new(0);
static CALLBACK_BUFFER: AtomicIsize = AtomicIsize::new(0);
static CALLBACK_VIEW: AtomicIsize = AtomicIsize::new(-1);
#[repr(C)]
struct Shortcut { ctrl: bool, alt: bool, shift: bool, key: u8 }
static mut SHORTCUT: Shortcut = Shortcut { ctrl: true, alt: true, shift: false, key: b'R' };
#[repr(C)]
struct Header { window: *mut c_void, id: usize, code: u32 }
#[link(name = "user32")]
unsafe extern "system" {
    fn IsWindow(window: *mut c_void) -> i32;
    fn SendMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
}
const fn label() -> [u16; 64] { let mut name = [0; 64]; name[0] = b'R' as u16; name[1] = b'u' as u16; name[2] = b'n' as u16; name }
static mut ITEMS: [FuncItem; 2] = [
    FuncItem { name: label(), function: Some(command), command_id: 0, checked: true, shortcut: std::ptr::addr_of_mut!(SHORTCUT).cast() },
    FuncItem { name: [0; 64], function: None, command_id: 0, checked: false, shortcut: std::ptr::null_mut() },
];
unsafe extern "C" fn command() {
    CALLS.fetch_add(1, Ordering::SeqCst);
    let host = HOST.load(Ordering::SeqCst) as *mut c_void;
    if !host.is_null() {
        CALLBACK_BUFFER.store(unsafe { SendMessageW(host, 0x400 + 1000 + 60, 0, 0) }, Ordering::SeqCst);
        let mut views = [-1i32, 0x12345678];
        let success = unsafe { SendMessageW(host, 0x400 + 1000 + 4, 0, views.as_mut_ptr() as isize) };
        let direct = unsafe { SendMessageW(host, 0x400 + 1000 + 88, 0, 0) };
        CALLBACK_VIEW.store(if success == 1 && views[1] == 0x12345678 && views[0] as isize == direct { direct } else { -1 }, Ordering::SeqCst);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn isUnicode() -> i32 { if cfg!(ansi) { 0 } else { 1 } }
#[unsafe(no_mangle)]
pub extern "C" fn getName() -> *const u16 { [70u16, 105, 120, 116, 117, 114, 101, 0].as_ptr() }
#[unsafe(no_mangle)]
pub extern "C" fn setInfo(data: NppData) {
    CALLS.store(10, Ordering::SeqCst);
    HOST.store(data.npp as usize, Ordering::SeqCst);
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn getFuncsArray(count: *mut i32) -> *mut FuncItem {
    unsafe { *count = if cfg!(bad_count) { -1 } else { 2 }; }
    std::ptr::addr_of_mut!(ITEMS).cast()
}
#[cfg(not(missing_message))]
#[unsafe(no_mangle)]
pub extern "C" fn messageProc(message: u32, _wparam: usize, _lparam: isize) -> isize {
    if message == 1 { unsafe { (*std::ptr::addr_of!(ITEMS))[0].command_id as isize } }
    else if message == 2 { LAST_NOTIFICATION.load(Ordering::SeqCst) }
    else if message == 3 { HOST_ALIVE.load(Ordering::SeqCst) }
    else if message == 4 { CALLBACK_BUFFER.load(Ordering::SeqCst) }
    else if message == 5 { CALLBACK_VIEW.load(Ordering::SeqCst) }
    else { CALLS.load(Ordering::SeqCst) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn beNotified(notification: *mut c_void) {
    unsafe {
        LAST_NOTIFICATION.store((*notification.cast::<Header>()).code as isize, Ordering::SeqCst);
        HOST_ALIVE.store(IsWindow(HOST.load(Ordering::SeqCst) as *mut c_void) as isize, Ordering::SeqCst);
    }
    CALLS.fetch_add(100, Ordering::SeqCst);
}
