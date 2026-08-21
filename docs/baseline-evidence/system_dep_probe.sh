#!/usr/bin/env bash
# Sonda de verificación empírica de las cuatro dependencias de sistema del paso
# "Install Linux system dependencies" de `.github/workflows/ci.yml`
# (tarea 14.1, Requisitos 10.4 y 10.5):
#
#   libwebkit2gtk-4.1-dev   libappindicator3-dev   librsvg2-dev   patchelf
#
# ---------------------------------------------------------------------------
# POR QUÉ LA SONDA NO ES LO QUE PIDE LITERALMENTE LA TAREA
#
# La tarea 14.1 describe el experimento como "quitar una dependencia del workflow y
# ejecutar `cargo check` / `cargo test`". Ese experimento requiere una corrida de CI, y
# este entorno no puede producir una: el Req 2.1-2.3 prohíbe commits y push, así que
# ningún cambio en `ci.yml` puede llegar a un runner. Además esta máquina no es
# `ubuntu-22.04` (es NixOS) y no tiene apt.
#
# La sonda ejecuta en cambio el experimento COMPLEMENTARIO, que es más fuerte para el
# caso "no es necesaria" y más débil para el caso "sí es necesaria":
#
#   en vez de quitar UNA dependencia de un entorno que tiene las cuatro,
#   se corre `cargo check --workspace` y `cargo test --workspace` en un entorno
#   donde NINGUNA de las cuatro está presente.
#
# Si el workspace compila y testea con las cuatro ausentes a la vez, entonces ninguna de
# las cuatro es necesaria para `cargo check`/`cargo test` en esta plataforma: el caso de
# "ninguna presente" implica los cuatro casos de "falta una". Lo que la sonda NO puede
# decidir es si alguna de las cuatro es necesaria en `ubuntu-22.04` específicamente, o
# para el empaquetado (`cargo packager`), que este entorno no ejecuta.
#
# Por eso el paso 1 de la sonda es verificar la ausencia: sin esa verificación, un
# `cargo check` exitoso no prueba nada.
# ---------------------------------------------------------------------------
#
# Uso:
#   bash docs/baseline-evidence/system_dep_probe.sh > docs/baseline-evidence/09-system-dep-probe.txt 2>&1
#
# La sonda es de solo lectura sobre el árbol de trabajo: no modifica `ci.yml`, no toca
# git y no instala ni desinstala nada.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/../.." || exit 1

# `PKG_CONFIG_PATH` de dbus + sqlite: es el ajuste SOLO DE ENTORNO ya registrado en
# `docs/product-audit.md` §3.8 y §4, sin el cual el baseline no compila en esta máquina
# por razones ajenas a las cuatro dependencias bajo análisis.
PKGCONF_DBUS=/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig
PKGCONF_SQLITE=/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig
export PKG_CONFIG_PATH="$PKGCONF_DBUS:$PKGCONF_SQLITE"

section() { printf '\n===== %s =====\n' "$1"; }

section "0. Entorno"
echo "fecha:  $(date -Is)"
echo "uname:  $(uname -srm)"
echo "distro: $(grep -E '^(NAME|VERSION_ID)=' /etc/os-release 2>/dev/null | tr '\n' ' ')"
echo "rustc:  $(rustc --version)"
echo "cargo:  $(cargo --version)"
echo "PKG_CONFIG_PATH=$PKG_CONFIG_PATH"
echo
echo "NOTA: el runner de CI es ubuntu-22.04. Esta máquina NO lo es, así que el resultado"
echo "no se transfiere a CI como una corrida equivalente; se transfiere como evidencia"
echo "sobre el grafo de dependencias de Cargo, que es el mismo en las dos máquinas."

section "1. Ausencia de las cuatro dependencias en esta máquina"
echo "-- apt/dpkg (los cuatro paquetes son paquetes Debian; sin dpkg no pueden estar instalados como tales) --"
if command -v dpkg >/dev/null 2>&1; then
  dpkg -l 2>/dev/null | grep -Ei 'webkit2gtk|appindicator|rsvg|patchelf' || echo "dpkg presente, ningún paquete coincidente instalado"
else
  echo "dpkg AUSENTE del PATH: esta máquina no tiene gestor de paquetes Debian"
fi

echo
echo "-- pkg-config: módulos que expondrían las bibliotecas de desarrollo --"
for mod in webkit2gtk-4.1 webkit2gtk-4.0 javascriptcoregtk-4.1 libsoup-3.0 \
           appindicator3-0.1 ayatana-appindicator3-0.1 librsvg-2.0 gtk+-3.0; do
  if pkg-config --exists "$mod" 2>/dev/null; then
    echo "  $mod: PRESENTE ($(pkg-config --modversion "$mod"))"
  else
    echo "  $mod: ausente"
  fi
done

echo
echo "-- patchelf como ejecutable en PATH --"
if command -v patchelf >/dev/null 2>&1; then
  echo "  patchelf PRESENTE: $(command -v patchelf) ($(patchelf --version 2>&1 | head -1))"
else
  echo "  patchelf AUSENTE del PATH"
fi

echo
echo "-- bibliotecas compartidas visibles para el enlazador dinámico --"
if command -v ldconfig >/dev/null 2>&1; then
  ldconfig -p 2>/dev/null | grep -Ei 'webkit|appindicator|rsvg' || echo "  ninguna coincidencia en la caché de ld"
else
  echo "  ldconfig ausente (esperado en NixOS: no hay caché global de ld)"
fi
for d in /usr/lib/x86_64-linux-gnu /usr/lib "$HOME/.nix-profile/lib"; do
  [ -d "$d" ] || continue
  hits=$(ls "$d" 2>/dev/null | grep -Ei 'webkit|appindicator|rsvg' | tr '\n' ' ')
  echo "  $d: ${hits:-sin coincidencias}"
done

section "2. Grafo de dependencias: qué bibliotecas de sistema puede necesitar el workspace"
echo "Declaraciones \`links\` de todos los paquetes del grafo resuelto (--all-features)."
echo "Un crate solo puede enlazar contra una biblioteca de sistema declarándola acá o"
echo "emitiendo flags desde su build script; \`links\` es la lista canónica."
cargo metadata --format-version 1 --all-features 2>/dev/null | python3 "$SCRIPT_DIR/system_dep_links.py"

echo
echo "-- crates de la era Tauri / pila WebKit / GTK buscados en Cargo.lock --"
pat='^name = "(.*(webkit|soup|javascriptcore|appindicator|rsvg|gtk|gdk|glib|gobject|cairo|pango|atk|tauri|wry|tao|patchelf|tray).*)"'
grep -nE "$pat" Cargo.lock || echo "  ninguna coincidencia: el grafo no contiene ningún crate de esa pila"

section "3. cargo check --workspace (con las cuatro dependencias ausentes)"
cargo check --workspace
echo "exit code cargo check: $?"

section "4. cargo test --workspace (con las cuatro dependencias ausentes)"
cargo test --workspace
echo "exit code cargo test: $?"

section "5. Alcance de lo que esta sonda NO decide"
cat <<'EOF'
1. No es una corrida de ubuntu-22.04. Si alguna de las cuatro fuera necesaria solo por
   una diferencia de esa imagen (y no por el grafo de Cargo), esta sonda no lo detecta.
2. No ejecuta `cargo packager`. El Req 10.5 conserva una dependencia si es necesaria para
   compilar, testear O EMPAQUETAR; el empaquetado no se mide acá.
3. `cargo check` y `cargo test` son headless y no abren ventana: no ejercitan la
   inicialización del backend gráfico en tiempo de ejecución.
EOF
