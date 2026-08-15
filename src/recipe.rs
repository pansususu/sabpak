use serde::{Deserialize, Serialize};

pub const RECIPES_DIR: &str = "recipes";
pub const FIRECIPES_DIR: &str = "firecipes";
pub const RELEASES_REPO: &str = "pansususu/packages";

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
}

#[derive(Serialize, Deserialize)]
pub struct Source {
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

pub fn load(nombre: &str) -> Recipe {
    let ruta = format!("{RECIPES_DIR}/{nombre}.toml");
    let contenido = std::fs::read_to_string(&ruta).unwrap_or_else(|_| {
        eprintln!("No existe la receta '{nombre}' en {RECIPES_DIR}/");
        std::process::exit(1);
    });
    toml::from_str(&contenido).unwrap_or_else(|e| {
        eprintln!("Receta inválida {ruta}: {e}");
        std::process::exit(1);
    })
}

/// Resuelve la ruta (relativa a `root`) del binario compilado: primero la
/// declarada en `out`, si no existe autodetecta en su carpeta.
pub fn find_binary(root: &str, out: &str, name: &str) -> String {
    if std::fs::metadata(format!("{root}/{out}")).is_ok() {
        return out.to_string();
    }
    let dir = match out.rsplit_once('/') {
        Some((d, _)) => format!("{root}/{d}"),
        None => root.to_string(),
    };
    let cands: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| !f.starts_with("lib") && !f.ends_with(".d") && !f.contains('.'))
        .collect();
    let rel = |f: &str| {
        let p = format!("{dir}/{f}");
        p.strip_prefix(root).unwrap_or(&p).trim_start_matches('/').to_string()
    };
    match cands.iter().find(|f| *f == name).or_else(|| cands.first()) {
        Some(f) => rel(f),
        None => {
            eprintln!("No se pudo autodetectar el binario en {dir} (esperaba '{out}')");
            std::process::exit(1);
        }
    }
}