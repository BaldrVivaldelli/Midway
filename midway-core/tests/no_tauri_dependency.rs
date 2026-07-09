//! Property 28: `midway-core` no depende de `tauri`
//! Validates: Requirements 1.10
//!
//! Este test es determinístico (no basado en generación aleatoria): el grafo de
//! dependencias de un `Cargo.lock` dado es fijo, por lo que basta con una única
//! aserción que recorra el subgrafo transitivo de `midway-core` dentro del
//! workspace y falle si aparece `tauri` o cualquier paquete `tauri-*`.
//!
//! La verificación está deliberadamente acotada al subgrafo de dependencias de
//! `midway-core` (no al workspace completo): crates hermanos como `midway`
//! (src-tauri) sí pueden depender de `tauri` sin que esta propiedad se vea
//! afectada.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::process::Command;

/// Ejecuta `cargo metadata --format-version 1` en la raíz del workspace y
/// devuelve el JSON parseado.
fn fetch_cargo_metadata() -> Value {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .current_dir(manifest_dir)
        .output()
        .expect("failed to execute `cargo metadata`");

    assert!(
        output.status.success(),
        "`cargo metadata` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("failed to parse `cargo metadata` JSON output")
}

/// Dado el JSON de `cargo metadata`, calcula el conjunto de nombres de paquete
/// en el subgrafo transitivo de dependencias de `midway-core` (incluyéndolo a
/// sí mismo), usando el grafo de resolución (`resolve.nodes`) en lugar de los
/// manifiestos declarados, para capturar también dependencias transitivas.
fn transitive_dependency_names(metadata: &Value, root_package_name: &str) -> HashSet<String> {
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata.packages should be an array");

    // Mapea package id -> nombre de paquete.
    let id_to_name: HashMap<String, String> = packages
        .iter()
        .map(|pkg| {
            let id = pkg["id"].as_str().expect("package.id should be a string");
            let name = pkg["name"]
                .as_str()
                .expect("package.name should be a string");
            (id.to_string(), name.to_string())
        })
        .collect();

    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("metadata.resolve.nodes should be an array");

    // Mapea package id -> lista de package ids de sus dependencias directas.
    let mut deps_by_id: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().expect("node.id should be a string");
        let deps = node["dependencies"]
            .as_array()
            .expect("node.dependencies should be an array")
            .iter()
            .map(|d| d.as_str().expect("dependency id should be a string").to_string())
            .collect::<Vec<_>>();
        deps_by_id.insert(id.to_string(), deps);
    }

    let root_id = id_to_name
        .iter()
        .find(|(_, name)| name.as_str() == root_package_name)
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("no se encontró el paquete `{root_package_name}` en cargo metadata"));

    // BFS/DFS sobre el grafo de resolución partiendo de `midway-core`.
    let mut visited_ids: HashSet<String> = HashSet::new();
    let mut stack = vec![root_id];

    while let Some(id) = stack.pop() {
        if !visited_ids.insert(id.clone()) {
            continue;
        }
        if let Some(deps) = deps_by_id.get(&id) {
            for dep_id in deps {
                if !visited_ids.contains(dep_id) {
                    stack.push(dep_id.clone());
                }
            }
        }
    }

    visited_ids
        .into_iter()
        .map(|id| {
            id_to_name
                .get(&id)
                .cloned()
                .unwrap_or_else(|| panic!("package id sin nombre asociado: {id}"))
        })
        .collect()
}

#[test]
fn midway_core_no_depende_de_tauri() {
    let metadata = fetch_cargo_metadata();
    let subgraph = transitive_dependency_names(&metadata, "midway-core");

    // Sanity check: el subgrafo de midway-core no debería estar vacío (al
    // menos debe contenerse a sí mismo), lo que confirma que el parseo del
    // grafo de resolución funcionó correctamente.
    assert!(
        subgraph.contains("midway-core"),
        "el subgrafo calculado no contiene a `midway-core`; revisar el parseo de cargo metadata"
    );

    let offending: Vec<&String> = subgraph
        .iter()
        .filter(|name| name.as_str() == "tauri" || name.starts_with("tauri-"))
        .collect();

    assert!(
        offending.is_empty(),
        "midway-core no debe depender de tauri, pero se encontraron los siguientes paquetes en su subgrafo de dependencias: {offending:?}"
    );
}
