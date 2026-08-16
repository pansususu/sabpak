use crate::{config, recipe, ver};
use std::process::Command;

/// Lista los tags de release publicados. `None` si `gh` no está disponible o
/// falla (para no abortar un lote).
fn list_tags(repo: &str) -> Option<Vec<String>> {
    let o = Command::new("gh")
        .args(["release", "list", "--repo", repo, "--json", "tagName", "--jq", ".[].tagName"])
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// Última versión publicada para `name` entre `tags`.
fn latest_version(name: &str, tags: &[String]) -> Option<String> {
    let prefix = format!("{name}-v");
    tags.iter()
        .filter(|t| t.starts_with(&prefix))
        .map(|t| t[prefix.len()..].to_string())
        .max_by(|a, b| ver::cmp(a, b))
}

/// Actualiza los paquetes de la lista de una sola pasada (un único `gh
/// release list` por lote) y reporta cuántos requirieron el cambio.
pub fn update_packages(pkgs: &[String]) {
    let repo = config::releases_repo();
    let Some(tags) = list_tags(&repo) else {
        eprintln!("No se pudieron consultar releases del repo {repo} (¿está instalado 'gh'?)");
        return;
    };
    for nombre in pkgs {
        update_one(nombre, &tags);
    }
}

fn update_one(nombre: &str, tags: &[String]) {
    let r = recipe::load(nombre);
    let name = r.package.name.clone();
    let installed =
        config::installed_version(&name).unwrap_or_else(|| r.package.version.clone());
    match latest_version(&name, tags) {
        None => println!("{name}: no hay releases publicadas"),
        Some(latest)
            if ver::cmp(&latest, &installed) == std::cmp::Ordering::Equal =>
        {
            println!("{name} ya está en la última versión ({installed})")
        }
        Some(latest) => {
            println!("Actualizando {name} {installed} -> {latest}");
            if crate::install::install_version(&name, Some(latest.clone())) {
                println!("{name} actualizado a {latest}");
            } else {
                eprintln!("{name}: falló la actualización a {latest}");
            }
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