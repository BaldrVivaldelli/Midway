#!/usr/bin/env python3
"""Lista las declaraciones `links` del grafo resuelto de Cargo.

Lee la salida de `cargo metadata --format-version 1` por stdin e imprime todos los
paquetes que declaran `links`, es decir los únicos que pueden enlazar de forma declarada
contra una biblioteca de sistema. Se usa en `system_dep_probe.sh` (tarea 14.1) para
mostrar que ninguna biblioteca de la pila WebKit/GTK aparece en el grafo.
"""

import json
import sys


def main() -> int:
    metadata = json.load(sys.stdin)
    packages = metadata["packages"]
    links = sorted({(p["name"], p["links"]) for p in packages if p.get("links")})

    print(f"  paquetes en el grafo: {len(packages)}")
    print(f"  paquetes con links:   {len(links)}")
    for name, lib in links:
        print(f"    {name} -> links = {lib}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
