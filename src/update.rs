use crate::{config, install, recipe, ver};
use std::process::Command;

fn list_tags(repo: &str) -> Vec<String> {
    let o = Command::new("gh")
        .args(["release", "list", "--repo", repo, "--json", "tagName", "--jq", ".[].tagName"])
        .output()
        .unwrap_or_else(|e| {
            eprintln!("gh no disponible: {e}");
            std::process::exit(1);
        });
    if !o.status.success() {
        eprintln!("gh falló al listar releases");
        std::process::exit(1);
    }
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Última versión publicada para `name` en el repo de releases.
fn latest_version(name: &str) -> Option<String> {
    let prefix = format!("{name}-v");
    list_tags(&config::releases_repo())
        .into_iter()
        .filter(|t| t.starts_with(&prefix))
        .map(|t| t[prefix.len()..].to_string())
        .max_by(|a, b| ver::cmp(a, b))
}

pub fn update_package(nombre: &str) {
    let r = recipe::load(nombre);
    let installed = config::installed_version(nombre).unwrap_or_else(|| r.package.version.clone());
    match latest_version(&r.package.name) {
        None => println!("{nombre}: no hay releases publicadas"),
        Some(latest) if latest == installed => {
            println!("{nombre} ya está en la última versión ({installed})")
        }
        Some(latest) => {
            println!("Actualizando {nombre} {installed} -> {latest}");
            install::install_version(nombre, Some(latest));
        }
    }
}

pub fn check_package(nombre: &str) {
    let Some(bin) = config::bin_name(nombre) else {
        println!("{nombre}: no está instalado");
        return;
    };
    let path = config::bin_dir().join(&bin);
    let ver = config::installed_version(nombre).unwrap_or_default();
    match std::fs::metadata(&path) {
        Ok(m) => {
            let exec = std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0;
            if exec {
                println!("{nombre} v{ver}: OK ({bin})");
            } else {
                println!("{nombre} v{ver}: {bin} existe pero NO es ejecutable");
            }
        }
        Err(_) => println!("{nombre} v{ver}: FALTA {bin} en {}", path.display()),
    }
}