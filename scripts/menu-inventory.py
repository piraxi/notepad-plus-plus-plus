"""Inventory source menu commands. References are not proof of implementation."""
import csv
import re
from pathlib import Path

root = Path(__file__).resolve().parents[1]
resource = (root / "notepad_plus_plus/PowerEditor/src/Notepad_plus.rc").read_text(encoding="utf-8-sig")
resource = resource.split("IDR_M30_MENU MENU", 1)[1].split("IDR_SYSTRAYPOPUP_MENU MENU", 1)[0]
app = (root / "Windows/src/app.rs").read_text(encoding="utf-8")
production = app.split("\n#[cfg(test)]\nmod tests", 1)[0]
dispatch = production.split("fn command(&mut self, command: u32)", 1)[1].split("fn drain(", 1)[0]
menus = production.split("unsafe fn create_menu()", 1)[1].split("pub fn run()", 1)[0]
languages = (root / "Windows/src/languages.rs").read_text(encoding="utf-8")
language_commands = set(re.findall(r"command: (IDM_LANG_\w+)", languages))
menus += languages.split("#[derive", 1)[0]
stack, pending, rows, seen = [], "", [], set()
for line in resource.splitlines():
    line = line.strip()
    popup = re.match(r'POPUP\s+"([^"]+)"', line)
    if popup:
        pending = popup.group(1).replace("&", "")
    elif line == "BEGIN":
        stack.append(pending)
        pending = ""
    elif line == "END":
        if stack:
            stack.pop()
    else:
        match = re.match(r'MENUITEM\s+"([^"]+)"\s*,\s*(\w+)', line)
        if not match:
            continue
        label, command = match.groups()
        path = " > ".join(filter(None, stack))
        if "Plugins" in path or "PLUGIN" in command or command in seen:
            continue
        seen.add(command)
        rows.append((path, label.replace("&", ""), command,
                     "yes" if command in language_commands or re.search(r"\b" + command + r"\b", menus) else "no",
                     "yes" if command in language_commands or re.search(r"\b" + command + r"\b", dispatch) else "no",
                     "Built-in lexer, syntax styles, folding, comments and completion; custom UDL styling pending" if command in language_commands else "Manual behavioral review required"))
destination = root / "docs/menu-inventory.csv"
with destination.open("w", newline="", encoding="utf-8") as output:
    writer = csv.writer(output)
    writer.writerow(("Menu", "Label", "Upstream command", "Referenced by menu builder", "Referenced by dispatcher", "Implementation notes"))
    writer.writerows(rows)
print(f"{len(rows)} distinct non-plugin commands; {sum(row[4] == 'no' for row in rows)} without a direct dispatcher reference. Dynamic commands and aliases require manual review.")
