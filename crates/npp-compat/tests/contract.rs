use npp_compat::commands::*;

#[test]
fn host_query_constants_match_the_supplied_header() {
    use npp_compat::messages::*;
    let source = include_str!(
        "../../../notepad_plus_plus/PowerEditor/src/MISC/PluginsManager/Notepad_plus_msgs.h"
    );
    assert_eq!(NPPMSG, 2024);
    for (name, value) in [
        ("NPPM_GETNBOPENFILES", NPPM_GETNBOPENFILES),
        ("NPPM_GETCURRENTDOCINDEX", NPPM_GETCURRENTDOCINDEX),
        ("NPPM_GETPOSFROMBUFFERID", NPPM_GETPOSFROMBUFFERID),
        ("NPPM_GETFULLPATHFROMBUFFERID", NPPM_GETFULLPATHFROMBUFFERID),
        ("NPPM_GETBUFFERIDFROMPOS", NPPM_GETBUFFERIDFROMPOS),
        ("NPPM_GETCURRENTBUFFERID", NPPM_GETCURRENTBUFFERID),
        ("NPPM_GETBUFFERENCODING", NPPM_GETBUFFERENCODING),
        ("NPPM_GETBUFFERFORMAT", NPPM_GETBUFFERFORMAT),
    ] {
        assert!(
            source
                .lines()
                .any(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")
                    == format!("#define {name} (NPPMSG + {})", value - NPPMSG)),
            "{name}"
        );
    }
    assert_eq!(RUNCOMMAND_USER, 4024);
    for (name, variable, value, offset) in [
        (
            "NPPM_GETFULLCURRENTPATH",
            "FULL_CURRENT_PATH",
            NPPM_GETFULLCURRENTPATH,
            1,
        ),
        (
            "NPPM_GETCURRENTDIRECTORY",
            "CURRENT_DIRECTORY",
            NPPM_GETCURRENTDIRECTORY,
            2,
        ),
        ("NPPM_GETFILENAME", "FILE_NAME", NPPM_GETFILENAME, 3),
        ("NPPM_GETNAMEPART", "NAME_PART", NPPM_GETNAMEPART, 4),
        ("NPPM_GETEXTPART", "EXT_PART", NPPM_GETEXTPART, 5),
        (
            "NPPM_GETCURRENTLINE",
            "CURRENT_LINE",
            NPPM_GETCURRENTLINE,
            8,
        ),
        (
            "NPPM_GETCURRENTCOLUMN",
            "CURRENT_COLUMN",
            NPPM_GETCURRENTCOLUMN,
            9,
        ),
        (
            "NPPM_GETCURRENTLINESTR",
            "CURRENT_LINESTR",
            NPPM_GETCURRENTLINESTR,
            12,
        ),
    ] {
        assert_eq!(value, RUNCOMMAND_USER + offset);
        let lines: Vec<_> = source
            .lines()
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        assert!(lines.contains(&format!("#define {variable} {offset}")));
        assert!(lines.contains(&format!("#define {name} (RUNCOMMAND_USER + {variable})")));
    }
}

#[test]
fn upstream_command_contract() {
    assert_eq!(IDM_FILE_NEW, 41001);
    assert_eq!(IDM_FILE_OPEN, 41002);
    assert_eq!(IDM_FILE_SAVE, 41006);
    assert!(ALL.len() > 400, "command inventory unexpectedly shrank");
    assert_eq!(
        ALL.iter()
            .find(|(name, _)| *name == "IDM_FILE_SAVE")
            .unwrap()
            .1,
        IDM_FILE_SAVE
    );
}

#[cfg(all(windows, target_pointer_width = "64"))]
#[test]
fn windows_x64_plugin_layout_matches_upstream_c_abi() {
    use npp_compat::plugin::*;
    use std::mem::{align_of, offset_of, size_of};
    assert_eq!(size_of::<NppData>(), 24);
    assert_eq!(size_of::<ShortcutKey>(), 4);
    assert_eq!(size_of::<FuncItem>(), 152);
    assert_eq!(align_of::<FuncItem>(), 8);
    assert_eq!(offset_of!(FuncItem, function), 128);
    assert_eq!(offset_of!(FuncItem, command_id), 136);
    assert_eq!(offset_of!(FuncItem, initially_checked), 140);
    assert_eq!(offset_of!(FuncItem, shortcut), 144);
    assert_eq!(size_of::<NotifyHeader>(), 24);
    assert_eq!(offset_of!(NotifyHeader, code), 16);
    assert_eq!(size_of::<ScNotification>(), 160);
    assert_eq!(offset_of!(ScNotification, position), 24);
    assert_eq!(offset_of!(ScNotification, text), 48);
    assert_eq!(offset_of!(ScNotification, wparam), 80);
    assert_eq!(offset_of!(ScNotification, annotation_lines_added), 136);
    assert_eq!(offset_of!(ScNotification, character_source), 152);
}

#[cfg(windows)]
#[test]
fn plugin_command_reservation_matches_supplied_resource_header() {
    use npp_compat::registry::{FIRST_PLUGIN_COMMAND, LAST_PLUGIN_COMMAND};
    let source = include_str!("../../../notepad_plus_plus/PowerEditor/src/resource.h");
    for (name, expected) in [
        ("ID_PLUGINS_CMD", FIRST_PLUGIN_COMMAND),
        ("ID_PLUGINS_CMD_LIMIT", LAST_PLUGIN_COMMAND),
    ] {
        let value = source.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            if words.next() == Some("#define") && words.next() == Some(name) {
                words.next()?.parse::<u16>().ok()
            } else {
                None
            }
        });
        assert_eq!(value, Some(expected));
    }
}
