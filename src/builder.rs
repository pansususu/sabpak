use serde::Deserialize;
use std::fs;
use std::process::Command;

#[derive(Deserialize)]
struct Recipe {
    package: Package,
    source: Source,
    build: Build,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct Source {
    url: String,
    tag: String,
}

#[derive(Deserialize)]
struct Build {
    #[serde(rename = "type")]
    kind: String,
    args: Vec<String>,
    output: String,
}

fn read_recipe(nombre: &str) -> Recipe {
    let ruta = format!("recipes/{nombre}.toml");
    let contenido = fs::read_to_string(&ruta).unwrap_or_else(|_| {
        eprintln!("No existe la receta '{nombre}' en recipes/");
        std::process::exit(1);
    });
    toml::from_str(&contenido).unwrap_or_else(|e| {
        eprintln!("Receta inválida {ruta}: {e}");
        std::process::exit(1);
    })
}

fn run(cmd: &str, args: &[&str], dir: Option<&str>) {
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    let status = c.status().unwrap_or_else(|e| {
        eprintln!("No se pudo ejecutar '{cmd}': {e}");
        std::process::exit(1);
    });
    if !status.success() {
        eprintln!("'{cmd}' falló con código {}", status.code().unwrap_or(-1));
        std::process::exit(1);
    }
}

pub fn build_package(nombre: &str) {
    let r = read_recipe(nombre);
    let workdir = format!("firecipes/tmp/{}-{}", r.package.name, r.package.version);
    let firecipes = "firecipes";
    fs::create_dir_all(firecipes).expect("No se pudo crear firecipes/");
    let _ = fs::remove_dir_all(&workdir);
    fs::create_dir_all(&workdir).expect("No se pudo crear el directorio de trabajo");

    run(
        "git",
        &["clone", "--depth", "1", "--branch", &r.source.tag, &r.source.url, &workdir],
        None,
    );

    match r.build.kind.as_str() {
        "cargo" => {
            let mut args = vec!["build"];
            args.extend(r.build.args.iter().map(String::as_str));
            run("cargo", &args, Some(&workdir));
        }
        "make" => run("make", &r.build.args.iter().map(String::as_str).collect::<Vec<_>>(), Some(&workdir)),
        other => {
            eprintln!("Tipo de build no soportado: '{other}'");
            std::process::exit(1);
        }
    }

    let artifact = format!("{workdir}/{}", r.build.output);
    if !fs::metadata(&artifact).is_ok() {
        eprintln!("No se encontró el artefacto esperado: {artifact}");
        std::process::exit(1);
    }

    let tarball = format!("{firecipes}/{}.tar.gz", r.package.name);
    run("tar", &["czf", &tarball, "-C", &workdir, &r.build.output], None);
    let _ = fs::remove_dir_all(&workdir);

    println!("Paquete listo: {tarball}");
}