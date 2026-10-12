#![cfg(windows)]

use npp_compat::{
    loader::LoadedPlugin,
    plugin::{NppData, ScNotification},
    registry::{FIRST_PLUGIN_COMMAND, LAST_PLUGIN_COMMAND, PluginRegistry, RegistrationError},
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

struct FixtureDirectory(PathBuf);
impl FixtureDirectory {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("npp-rust-plugin-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn build(&self, variant: &str) -> PathBuf {
        let output = self.0.join(format!("{variant}.dll"));
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin.rs");
        let result = Command::new("rustc")
            .arg("--crate-type=cdylib")
            .arg("--edition=2024")
            .arg("--cfg")
            .arg(variant)
            .arg(source)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "fixture compilation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        output
    }
}
impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        // The uniquely created directory contains only this test's compiler outputs.
        // Remove individual files, then the directory; never recurse into other paths.
        for entry in std::fs::read_dir(&self.0).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}
fn handles() -> NppData {
    NppData {
        npp_handle: std::ptr::null_mut(),
        scintilla_main_handle: std::ptr::null_mut(),
        scintilla_second_handle: std::ptr::null_mut(),
    }
}

#[test]
fn compiled_plugin_exports_commands_messages_and_unloads() {
    let directory = FixtureDirectory::new();
    let path = directory.build("valid");
    // SAFETY: This test compiles and owns the fixture source. It ignores host
    // handles, creates no windows/threads, and keeps static ABI data until unload.
    let mut plugin = unsafe { LoadedPlugin::load(&path, handles()) }.unwrap();
    assert_eq!(plugin.name(), "Fixture");
    assert_eq!(plugin.menu_items().len(), 2);
    assert_eq!(plugin.menu_items()[0].name, "Run");
    assert!(plugin.menu_items()[0].initially_checked);
    assert!(!plugin.menu_items()[0].is_separator);
    assert!(plugin.menu_items()[1].is_separator);
    plugin.assign_command_id(0, 62000).unwrap();
    assert!(plugin.assign_command_id(2, 62001).is_err());
    // SAFETY: Fixture messages use no pointers; its notification reads the supplied
    // header and command only increments a counter. No callbacks are retained.
    unsafe {
        assert_eq!(plugin.message(1, 0, 0), 62000);
        assert_eq!(plugin.message(0, 0, 0), 10);
        assert!(plugin.invoke(0).unwrap());
        assert!(!plugin.invoke(1).unwrap());
        assert!(plugin.invoke(2).is_err());
        // Provide initialized writable notification storage for the callback.
        let mut notification = ScNotification::default();
        plugin.notify(&mut notification);
        assert_eq!(plugin.message(0, 0, 0), 111);
    }
    drop(plugin);
    // Windows prevents deleting a DLL still mapped by this process.
    std::fs::remove_file(path).unwrap();
}

#[test]
fn rejected_plugins_release_their_library_reference() {
    let directory = FixtureDirectory::new();
    for (variant, message) in [
        ("missing_message", "messageProc"),
        ("ansi", "ANSI"),
        ("bad_count", "command array"),
    ] {
        let path = directory.build(variant);
        // SAFETY: Same controlled fixture contract as the successful-load test;
        // these variants deliberately omit an export or return invalid metadata.
        let result = unsafe { LoadedPlugin::load(&path, handles()) };
        let error = match result {
            Ok(_) => panic!("invalid fixture accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains(message), "{error}");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn registry_routes_commands_to_their_own_dll_and_broadcasts_notifications() {
    let directory = FixtureDirectory::new();
    let first_path = directory.build("first");
    let second_path = directory.build("second");
    let mut registry = PluginRegistry::default();
    // SAFETY: Controlled fixtures use static ABI data and no host handles or threads.
    unsafe {
        registry
            .register(LoadedPlugin::load(&first_path, handles()).unwrap())
            .unwrap();
        registry
            .register(LoadedPlugin::load(&second_path, handles()).unwrap())
            .unwrap();
        assert_eq!(registry.plugins().len(), 2);
        assert_eq!(registry.plugins()[0].name(), "Fixture");
        assert_eq!(
            registry.plugins()[0]
                .menu_items()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![Some(FIRST_PLUGIN_COMMAND), None]
        );
        assert_eq!(
            registry.plugins()[1]
                .menu_items()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![Some(FIRST_PLUGIN_COMMAND + 1), None]
        );
        assert_eq!(
            registry.message(0, 1, 0, 0),
            Some(FIRST_PLUGIN_COMMAND as isize)
        );
        assert_eq!(
            registry.message(1, 1, 0, 0),
            Some(FIRST_PLUGIN_COMMAND as isize + 1)
        );
        assert!(registry.dispatch(FIRST_PLUGIN_COMMAND + 1));
        assert_eq!(registry.message(0, 0, 0, 0), Some(10));
        assert_eq!(registry.message(1, 0, 0, 0), Some(11));
        assert!(!registry.dispatch(FIRST_PLUGIN_COMMAND - 1));
        assert!(!registry.dispatch(LAST_PLUGIN_COMMAND));
        assert_eq!(registry.message(2, 0, 0, 0), None);
        registry.notify_all(&mut ScNotification::default());
        assert_eq!(registry.message(0, 0, 0, 0), Some(110));
        assert_eq!(registry.message(1, 0, 0, 0), Some(111));
    }
    drop(registry);
    std::fs::remove_file(first_path).unwrap();
    std::fs::remove_file(second_path).unwrap();
}

#[test]
fn registry_preflights_capacity_and_duplicate_modules_without_losing_ownership() {
    let directory = FixtureDirectory::new();
    let first_path = directory.build("first");
    let second_path = directory.build("second");
    let mut registry = PluginRegistry::with_command_range(u16::MAX..=u16::MAX);
    // SAFETY: All variants are controlled fixtures with static data and no threads.
    unsafe {
        registry
            .register(LoadedPlugin::load(&first_path, handles()).unwrap())
            .unwrap();
        assert!(registry.dispatch(u16::MAX));
        assert!(!registry.dispatch(0));
        let rejected = registry
            .register(LoadedPlugin::load(&second_path, handles()).unwrap())
            .unwrap_err();
        assert_eq!(rejected.reason, RegistrationError::CommandRangeFull);
        assert_eq!(rejected.plugin.message(1, 0, 0), 0);
        assert_eq!(registry.plugins().len(), 1);
        // The caller retains the rejected DLL and can still send shutdown messages.
        assert_eq!(rejected.plugin.message(0, 0, 0), 10);
        drop(rejected);
        std::fs::remove_file(second_path).unwrap();
        let duplicate = registry
            .register(LoadedPlugin::load(&first_path, handles()).unwrap())
            .unwrap_err();
        assert_eq!(duplicate.reason, RegistrationError::DuplicateModule);
        drop(duplicate);
        assert_eq!(registry.message(0, 1, 0, 0), Some(u16::MAX as isize));
        assert!(registry.dispatch(u16::MAX));
    }
    drop(registry);
    std::fs::remove_file(first_path).unwrap();
}
