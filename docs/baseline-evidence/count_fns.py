import re
import sys

path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
pat = re.compile(r"^(pub )?fn (\w+)")
for i, line in enumerate(lines):
    m = pat.match(line)
    if not m:
        continue
    name = m.group(2)
    if not (name.startswith("update_") or name in ("update", "view")):
        continue
    depth = 0
    opened = False
    j = i
    while j < len(lines):
        depth += lines[j].count("{") - lines[j].count("}")
        if "{" in lines[j]:
            opened = True
        if opened and depth <= 0:
            break
        j += 1
    print(f"{name}: {i + 1}-{j + 1} ({j - i + 1} líneas)")
