fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build_editor();
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("app.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rerun-if-changed=app.manifest");
    }
}

fn sources(directory: &str) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "cxx"))
        .collect();
    files.sort();
    files
}
fn build_editor() {
    let root = "../notepad_plus_plus";
    println!("cargo:rerun-if-changed={root}/scintilla");
    println!("cargo:rerun-if-changed={root}/lexilla");
    println!("cargo:rerun-if-changed={root}/boostregex");
    let mut sci = cc::Build::new();
    sci.cpp(true)
        .std("c++17")
        .warnings(false)
        .flag_if_supported("/utf-8")
        .define("SCI_NAMESPACE", None)
        .define("SCI_OWNREGEX", None)
        .define("BOOST_REGEX_STANDALONE", None)
        .define("_CRT_SECURE_NO_DEPRECATE", None)
        .include(format!("{root}/scintilla/include"))
        .include(format!("{root}/scintilla/src"))
        .include(format!("{root}/boostregex"))
        .files(sources(&format!("{root}/scintilla/src")))
        .files(sources(&format!("{root}/boostregex")));
    for name in [
        "HanjaDic",
        "PlatWin",
        "ListBox",
        "SurfaceGDI",
        "SurfaceD2D",
        "ScintillaWin",
    ] {
        sci.file(format!("{root}/scintilla/win32/{name}.cxx"));
    }
    sci.compile("npp_scintilla");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .flag_if_supported("/utf-8")
        .define("LEXILLA_NO_EXPORT", None)
        .define("_CRT_SECURE_NO_DEPRECATE", None)
        .include(format!("{root}/scintilla/include"))
        .include(format!("{root}/lexilla/include"))
        .include(format!("{root}/lexilla/lexlib"))
        .files(sources(&format!("{root}/lexilla/lexlib")))
        .files(sources(&format!("{root}/lexilla/lexers")))
        .file(format!("{root}/lexilla/src/Lexilla.cxx"))
        .compile("npp_lexilla");
    for library in [
        "imm32", "ole32", "oleaut32", "gdi32", "user32", "advapi32", "uuid", "msimg32",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
    // Numeric messages are generated from the exact bundled renderer version.
    let source = std::fs::read_to_string(format!("{root}/scintilla/include/Scintilla.h")).unwrap();
    let mut constants = String::new();
    for line in source.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() == 3
            && parts[0] == "#define"
            && (parts[1].starts_with("SCI_")
                || parts[1].starts_with("SC_")
                || parts[1].starts_with("SCN_")
                || parts[1].starts_with("STYLE_"))
        {
            let number = parts[2].parse::<i64>().ok().or_else(|| {
                parts[2]
                    .strip_prefix("0x")
                    .and_then(|n| i64::from_str_radix(n, 16).ok())
            });
            if let Some(number) = number {
                constants.push_str(&format!(
                    "pub const {}: u32 = {}u32;\n",
                    parts[1], number as u32
                ));
            }
        }
    }
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("scintilla_ids.rs"),
        constants,
    )
    .unwrap();
}
