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

fn ensure_recipes_dir() {
    match fs::metadata(recipe::RECIPES_DIR) {
        Ok(m) if m.is_dir() => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {
            if let Err(e) = fs::create_dir(recipe::RECIPES_DIR) {
                eprintln!("No se pudo crear {}/: {e}", recipe::RECIPES_DIR);
                exit(1);
            }
        }
        Ok(_) => {
            eprintln!("{} existe pero no es una carpeta", recipe::RECIPES_DIR);
            exit(1);
        }
        Err(e) => {
            eprintln!("No se pudo acceder a {}: {e}", recipe::RECIPES_DIR);
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
    let receta = recipe::Recipe {
        package: recipe::Package { name: nombre.into(), version },
        source: recipe::Source { url: format!("{}.git", repo.html_url), tag },
        build: recipe::Build {
            kind: "cargo".into(),
            args: vec!["--release".into()],
            output: format!("target/release/{nombre}"),
        },
    };
    ensure_recipes_dir();
    fs::write(
        format!("{}/{nombre}.toml", recipe::RECIPES_DIR),
        toml::to_string(&receta).unwrap_or_else(|e| {
            eprintln!("No se pudo serializar la receta: {e}");
            exit(1)
        }),
    )
    .unwrap_or_else(|e| {
        eprintln!("No se pudo escribir la receta: {e}");
        exit(1)
    });
    println!(
        "Receta creada: {nombre} -> {} ({} estrellas)",
        repo.full_name, repo.stargazers_count
    );
}