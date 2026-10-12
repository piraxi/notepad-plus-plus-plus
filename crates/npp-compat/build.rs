use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    let header = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../notepad_plus_plus/PowerEditor/src/menuCmdID.h");
    println!("cargo:rerun-if-changed={}", header.display());
    let source =
        fs::read_to_string(header).expect("provided upstream menu command header must exist");
    let mut values = BTreeMap::<String, u32>::new();
    let mut pending = Vec::new();
    for line in source.lines() {
        let Some(line) = line.trim().strip_prefix("#define") else {
            continue;
        };
        let line = line.split("//").next().unwrap().trim();
        let Some(index) = line.find(char::is_whitespace) else {
            continue;
        };
        let (name, expression) = line.split_at(index);
        pending.push((name.to_owned(), expression.trim().to_owned()));
    }
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|(name, expression)| {
            let expression = expression.replace(['(', ')'], "");
            let sum = expression.split('+').try_fold(0u32, |sum, term| {
                let term = term.trim();
                let value = term
                    .parse::<u32>()
                    .ok()
                    .or_else(|| values.get(term).copied())?;
                sum.checked_add(value)
            });
            if let Some(value) = sum {
                values.insert(name.clone(), value);
                false
            } else {
                true
            }
        });
        assert!(
            pending.len() < before,
            "unsupported or unresolved upstream command expressions: {pending:?}"
        );
    }
    let mut generated = String::from("// Generated from the supplied Notepad++ menuCmdID.h.\n");
    for (name, value) in &values {
        generated.push_str(&format!("pub const {name}: u32 = {value};\n"));
    }
    generated.push_str("pub const ALL: &[(&str, u32)] = &[\n");
    for (name, value) in &values {
        generated.push_str(&format!("    (\"{name}\", {value}),\n"));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("commands.rs"),
        generated,
    )
    .unwrap();
}
