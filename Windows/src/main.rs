#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod case_conversion;
#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod date_time;
#[cfg(windows)]
mod editor_chrome;
#[cfg(windows)]
mod file_attributes;
#[cfg(windows)]
mod file_drop;
#[cfg(windows)]
mod file_io;
#[cfg(windows)]
mod file_launch;
#[cfg(windows)]
mod find_files;
#[cfg(windows)]
mod formatting;
#[cfg(windows)]
mod goto_dialog;
#[cfg(windows)]
mod input_dialog;
#[cfg(windows)]
mod languages;
#[cfg(windows)]
mod locale_sort;
#[cfg(windows)]
mod macros;
#[cfg(windows)]
mod path_clipboard;
#[cfg(windows)]
mod plugin_accelerators;
#[cfg(windows)]
mod plugin_host;
#[cfg(windows)]
mod plugin_menu;
#[cfg(windows)]
mod preferences;
#[cfg(windows)]
mod printing;
#[cfg(windows)]
mod projection;
#[cfg(windows)]
mod scintilla;
#[cfg(windows)]
mod search_dialog;
#[cfg(windows)]
mod summary;
#[cfg(windows)]
mod syntax_data;
#[cfg(windows)]
mod tabs;

#[cfg(windows)]
fn main() {
    if let Err(error) = app::run() {
        eprintln!("Cannot start editor: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This executable is the Windows adapter. Other platforms are not implemented yet.");
}
