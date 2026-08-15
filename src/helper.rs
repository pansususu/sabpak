use serde::Deserialize;
use std::fs::create_dir_all;

#[derive(Deserialize)]
struct Repo {
    full_name: String,
    stargazers_count: i64,
    html_url: String,
    default_branch: String,
}

#[derive(Deserialize)]
struct SearchResults {
    items: Vec<Repo>,
}

fn search_github(nombre: &str) -> Option<Repo> {
    let url = format!(
        "https://api.github.com/search/repositories?q={nombre}&sort=stars&order=desc"
    );
    let resultados: SearchResults = ureq::get(&url)
        .set("User-Agent", "sabpak")
        .call()
        .ok()?
        .into_json()
        .ok()?;
    resultados
        .items
        .into_iter()
        .max_by_key(|repo| repo.stargazers_count)
}

fn crear_receta(nombre: &str, repo: &Repo) {
    create_dir_all("recipes").expect("No se pudo crear la carpeta recipes");
    let contenido = format!(
        "[package]\nname = \"{nombre}\"\nversion = \"0.1.0\"\n\n\
         [source]\nurl = \"{}.git\"\ntag = \"{}\"\n\n\
         [build]\ntype = \"cargo\"\nargs = [\"--release\"]\noutput = \"target/release/{nombre}\"\n",
        repo.html_url, repo.default_branch
    );
    std::fs::write(format!("recipes/{nombre}.toml"), contenido)
        .expect("No se pudo escribir la receta");
}

pub fn new_recipe(receta: &str) {
    match search_github(receta) {
        Some(repo) => {
            crear_receta(receta, &repo);
            println!(
                "Receta creada: {receta} -> {} ({} estrellas)",
                repo.full_name, repo.stargazers_count
            );
        }
        None => println!("No se encontró ningún proyecto llamado '{receta}' en GitHub"),
    }
}
