use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::Metadata;
use std::path::PathBuf;

use crate::config;

/// true si `m` es un archivo regular con algún bit de ejecución.
pub fn is_exec_file(m: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.is_file() && m.permissions().mode() & 0o111 != 0
}

/// true si la ruta apunta a un archivo regular ejecutable.
fn is_exec_path(p: &str) -> bool {
    std::fs::metadata(p).map(|m| is_exec_file(&m)).unwrap_or(false)
}

pub fn recipes_dir() -> PathBuf {
    config::base_dir().join("recipes")
}

pub fn firecipes_dir() -> PathBuf {
    config::base_dir().join("firecipes")
}

#[derive(Serialize, Deserialize)]
pub struct Recipe {
    pub package: Package,
    pub source: Source,
    pub build: Build,
}

#[derive(Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub deps: Vec<String>,
}

fn default_kind() -> String {
    "git".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct Source {
    #[serde(rename = "type", default = "default_kind")]
    pub kind: String,
    pub url: String,
    pub tag: String,
}

#[derive(Serialize, Deserialize)]
pub struct Build {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub output: String,
}

pub fn tarball_name(r: &Recipe) -> String {
    format!("{}-{}.tar.gz", r.package.name, r.package.version)
}

pub fn release_tag(r: &Recipe) -> String {
    format!("{}-v{}", r.package.name, r.package.version)
}

/// Resolución de dependencias en orden topológico (las dependencias primero,
/// el paquete objetivo al final). Detecta ciclos.
pub fn resolve(nombre: &str) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut state: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    resolve_dfs(nombre, &mut state, &mut order);
    order
}

fn resolve_dfs(
    nombre: &str,
    state: &mut std::collections::HashMap<String, u8>,
    order: &mut Vec<String>,
) {
    match state.get(nombre).copied() {
        Some(2) => return, // ya resuelto
        Some(1) => {
            // Ciclo: no lo marcamos como resuelto para no incluirlo en el
            // orden, pero no abortamos el resto del lote.
            eprintln!("Ciclo de dependencias detectado en '{nombre}' (se omite)");
            return;
        }
        _ => {}
    }
    state.insert(nombre.to_string(), 1);
    let r = load(nombre);
    for dep in &r.package.deps {
        resolve_dfs(dep, state, order);
    }
    state.insert(nombre.to_string(), 2);
    order.push(nombre.to_string());
}

pub fn load(nombre: &str) -> Recipe {
    let ruta = recipes_dir().join(format!("{nombre}.toml"));
    let contenido = std::fs::read_to_string(&ruta).unwrap_or_else(|_| {
        eprintln!("No existe la receta '{nombre}' en {}/", recipes_dir().display());
        std::process::exit(1);
    });
    toml::from_str(&contenido).unwrap_or_else(|e| {
        eprintln!("Receta inválida {}: {e}", ruta.display());
        std::process::exit(1);
    })
}

/// Resuelve la ruta (relativa a `root`) del binario compilado: primero la
/// declarada en `out`; si no existe, busca un ejecutable llamado como el
/// paquete debajo de la carpeta de salida; como último recurso toma cualquier
/// ejecutable de esa carpeta. Devuelve `None` si no hay ninguno.
pub fn find_binary(root: &str, out: &str, name: &str) -> Option<String> {
    let exact = format!("{root}/{out}");
    if is_exec_path(&exact) {
        return Some(out.to_string());
    }
    let dir = out.rsplit_once('/').map(|(d, _)| d).unwrap_or("").to_string();
    let base = format!("{root}/{dir}");
    let rel = |p: std::path::PathBuf| {
        p.strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .into_owned()
    };
    // Todos los paths relativos que son ejecutables regulares.
    let mut executables: Vec<String> = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(&base)];
    // Protectores contra symlink-ciclos (tarballs maliciosos o rotos).
    let mut seen: HashSet<std::path::PathBuf> = HashSet::new();
    while let Some(d) = stack.pop() {
        // Resolvemos el directorio real para no re-escarbar ciclos.
        let real = std::fs::canonicalize(&d).unwrap_or_else(|_| d.clone());
        if !seen.insert(real) {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            // seguimos solo directorios que no son enlaces simbólicos
            // (evita seguir symlinks arbitrarios del tarball).
            if let Ok(mt) = p.symlink_metadata() {
                if mt.file_type().is_dir() {
                    stack.push(p);
                } else if mt.file_type().is_file() && let Ok(m) = p.metadata() {
                    if is_exec_file(&m) {
                        executables.push(rel(p));
                    }
                }
            }
        }
    }
    if executables.contains(&format!("{dir}/{name}")) {
        return Some(format!("{dir}/{name}"));
    }
    if let Some(f) = executables.iter().find(|f| f.ends_with(&format!("/{name}"))) {
        return Some(f.clone());
    }
    executables.first().cloned()
}