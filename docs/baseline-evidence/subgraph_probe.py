"""Recorre `resolve.nodes` de `cargo metadata` y cuenta el subgrafo transitivo
de cada miembro del workspace, reportando los paquetes de la familia iced y tauri.

Uso (desde la raíz del repo):
    python3 docs/baseline-evidence/subgraph_probe.py
"""

import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

env = dict(os.environ)
env.setdefault(
    "PKG_CONFIG_PATH",
    "/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig"
    ":/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig",
)

out = subprocess.run(
    ["cargo", "metadata", "--format-version", "1"],
    capture_output=True,
    env=env,
    cwd=REPO,
)
if out.returncode != 0:
    print("ERROR", out.stderr.decode()[-3000:])
    sys.exit(1)

md = json.loads(out.stdout)
id_to_name = {p["id"]: p["name"] for p in md["packages"]}
deps = {n["id"]: list(n["dependencies"]) for n in md["resolve"]["nodes"]}


def subgraph(root):
    root_id = next(i for i, n in id_to_name.items() if n == root)
    seen, stack = set(), [root_id]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in deps.get(current, []):
            if dep not in seen:
                stack.append(dep)
    return sorted({id_to_name[i] for i in seen})


for root in ("midway-core", "midway-desktop"):
    names = subgraph(root)
    iced = [n for n in names if n == "iced" or n.startswith("iced")]
    tauri = [n for n in names if n == "tauri" or n.startswith("tauri-")]
    print(f"{root}: total={len(names)} iced={len(iced)} {iced} tauri={len(tauri)}")
