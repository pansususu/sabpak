use crate::recipe;
use serde::de::DeserializeOwned;
use std::{fs, io::ErrorKind, process::exit};

#[derive(serde::Deserialize)]
struct Repo {
    full_name: String,
    stargazers_count: i64,
    html_url: String,
    default_branch: String,
}

#[derive(serde::Deserialize)]
struct Search {
    items: Vec<Repo>,
}

#[derive(serde::Deserialize)]
struct Latest {
    tag_name: String,
}

#[derive(serde::Deserialize)]
struct ContentItem {
    name: String,
}

fn get<T: DeserializeOwned>(url: &str) -> Option<T> {
    ureq::get(url).set("User-Agent", "sabpak").call().ok()?.into_json().ok()
}

fn search(nombre: &str) -> Option<Repo> {
    let url = format!("https://api.github.com/search/repositories?q={nombre}&sort=stars&order=desc");
    let search: Search = get(&url)?;
    search.items.into_iter().max_by_key(|r| r.stargazers_count)
}

fn latest_tag(repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let latest: Latest = get(&url)?;
    Some(latest.tag_name)
}

/// Lista los nombres de archivos en la raíz de la rama por defecto.
fn root_files(repo: &str, branch: &str) -> Vec<String> {
    let url = format!("https://api.github.com/repos/{repo}/contents?ref={branch}");
    let items: Option<Vec<ContentItem>> = get(&url);
    items.unwrap_or_default().into_iter().map(|i| i.name).collect()
}

/// Detecta el build system a partir de las pistas de la raíz del repo.
/// Devuelve `(tipo, argumentos, salida esperada)`.
fn detect_build(nombre: &str, files: &[String]) -> (String, Vec<String>, String) {
    let has = |pat: &str| files.iter().any(|f| f.eq_ignore_ascii_case(pat));
    if has("Cargo.toml") {
        ("cargo".into(), vec!["--release".into()], format!("target/release/{nombre}"))
    } else if has("CMakeLists.txt") {
        ("cmake".into(), vec![], format!("build/{nombre}"))
    } else if has("Makefile") || has("makefile") || has("GNUmakefile") {
        ("make".into(), vec![], nombre.into())
    } else {
        ("cargo".into(), vec!["--release".into()], format!("target/release/{nombre}"))
    }
}

fn ensure_recipes_dir() {
    let dir = recipe::recipes_dir();
    match fs::metadata(&dir) {
        Ok(m) if m.is_dir() => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {
            if let Err(e) = fs::create_dir_all(&dir) {
                eprintln!("No se pudo crear {}/: {e}", dir.display());
                exit(1);
            }
        }
        Ok(_) => {
            eprintln!("{} existe pero no es una carpeta", dir.display());
            exit(1);
        }
        Err(e) => {
            eprintln!("No se pudo acceder a {}: {e}", dir.display());
            exit(1);
        }
    }
}

pub fn new_recipe(nombre: &str) {
    let Some(repo) = search(nombre) else {
        println!("No se encontró ningún proyecto llamado '{nombre}' en GitHub");
        return;
    };
    // Último tag de release si existe; si no, rama por defecto.
    let tag = latest_tag(&repo.full_name).unwrap_or(repo.default_branch.clone());
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    // Busca pistas para detectar el build system (cargo / cmake / make).
    let branch = repo.default_branch.clone();
    let files = root_files(&repo.full_name, &branch);
    let (kind, args, output) = detect_build(nombre, &files);
    println!(
        "Receta creada: {nombre} -> {} ({} estrellas) [build: {kind}]",
        repo.full_name, repo.stargazers_count
    );
    let receta = recipe::Recipe {
        package: recipe::Package { name: nombre.into(), version, deps: Vec::new() },
        source: recipe::Source { kind: "git".to_string(), url: format!("{}.git", repo.html_url), tag },
        build: recipe::Build {
            kind,
            args,
            output,
        },
    };
    ensure_recipes_dir();
    fs::write(
        recipe::recipes_dir().join(format!("{nombre}.toml")),
        toml::to_string(&receta).unwrap_or_else(|e| {
            eprintln!("No se pudo serializar la receta: {e}");
            exit(1)
        }),
    )
    .unwrap_or_else(|e| {
        eprintln!("No se pudo escribir la receta: {e}");
        exit(1)
    });
}