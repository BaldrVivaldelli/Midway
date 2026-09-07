import re
import sys

TARGETS = [
    "Message",
    "RequestComposerMessage",
    "ResponseInspectorMessage",
    "WorkspaceMessage",
    "RunnerMessage",
    "PaletteMessage",
    "ThemeMessage",
    "SessionMessage",
    "UpdaterMessage",
    "KeyboardMessage",
    "ActivityBarMessage",
    "TreeMessage",
    "WorkspaceCrudMessage",
    "TopBarMessage",
    "PanelResizeMessage",
]


def count(path):
    lines = open(path, encoding="utf-8").read().splitlines()
    for name in TARGETS:
        pat = re.compile(r"^pub enum " + name + r" \{$")
        for i, line in enumerate(lines):
            if pat.match(line):
                depth = 1  # already inside the enum body
                variants = 0
                j = i + 1
                while j < len(lines):
                    stripped = lines[j].strip()
                    if depth == 1 and re.match(r"^[A-Z][A-Za-z0-9_]*(\(|\s*\{|,|$)", stripped):
                        variants += 1
                    depth += lines[j].count("{") - lines[j].count("}")
                    if depth <= 0:
                        break
                    j += 1
                print(f"{name}: file={path} decl_line={i + 1} end_line={j + 1} variants={variants}")


for p in sys.argv[1:]:
    count(p)
