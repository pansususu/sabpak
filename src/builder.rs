use crate::recipe;
use std::{fs, process::Command};

fn run(cmd: &str, args: &[&str], dir: Option<&str>) {
    if !run_ok(cmd, args, dir) {
        std::process::exit(1);
    }
}

fn run_ok(cmd: &str, args: &[&str], dir: Option<&str>) -> bool {
    let status = match dir {
        Some(d) => Command::new(cmd).args(args).current_dir(d).status(),
        None => Command::new(cmd).args(args).status(),
    };
    let status = match status {
        Ok(s) => s,
        Err(e) => {
            eprintln!("No se pudo ejecutar '{cmd}': {e}");
            return false;
        }
    };
    if status.success() {
        return true;
    }
    eprintln!("'{cmd}' falló con código {}", status.code().unwrap_or(-1));
    false
}

fn out(cmd: &str, args: &[&str]) -> String {
    let o = Command::new(cmd).args(args).output().unwrap_or_else(|e| {
        eprintln!("No se pudo ejecutar '{cmd}': {e}");
        std::process::exit(1);
    });
    if !o.status.success() {
        eprintln!("'{cmd}' falló con código {}", o.status.code().unwrap_or(-1));
        std::process::exit(1);
    }
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

pub fn build_package(nombre: &str) {
    let r = recipe::load(nombre);
    let workdir = format!("{}/tmp/{}-{}", recipe::FIRECIPES_DIR, r.package.name, r.package.version);
    fs::create_dir_all(recipe::FIRECIPES_DIR).unwrap_or_else(|e| {
        eprintln!("No se pudo crear {}: {e}", recipe::FIRECIPES_DIR);
        std::process::exit(1);
    });
    let _ = fs::remove_dir_all(&workdir);
    fs::create_dir_all(&workdir).unwrap_or_else(|e| {
        eprintln!("No se pudo crear el directorio de trabajo {workdir}: {e}");
        std::process::exit(1);
    });

    run("git", &["clone", "--depth", "1", "--branch", &r.source.tag, &r.source.url, &workdir], None);

    match r.build.kind.as_str() {
        "cargo" => {
            let mut args = vec!["build"];
            args.extend(r.build.args.iter().map(String::as_str));
            run("cargo", &args, Some(&workdir));
        }
        "make" => {
            let args: Vec<&str> = r.build.args.iter().map(String::as_str).collect();
            run("make", &args, Some(&workdir));
        }
        other => {
            eprintln!("Tipo de build no soportado: '{other}'");
            std::process::exit(1);
        }
    }

    let bin_rel = recipe::find_binary(&workdir, &r.build.output, &r.package.name);
    let tarball = format!("{}/{}", recipe::FIRECIPES_DIR, recipe::tarball_name(&r));
    run("tar", &["czf", &tarball, "-C", &workdir, &bin_rel], None);
    let _ = fs::remove_dir_all(&workdir);
    println!("Paquete listo: {tarball}");

    let repo = recipe::RELEASES_REPO;
    let tag = recipe::release_tag(&r);
    let prefix = format!("{}-v", r.package.name);
    for t in out("gh", &["release", "list", "--repo", repo,
        "--json", "tagName", "--jq", ".[].tagName"]).lines()
    {
        let t = t.trim();
        if t != tag && t.starts_with(&prefix) {
            run("gh", &["release", "delete", t, "--repo", repo, "--yes", "--cleanup-tag"], None);
            println!("Eliminada release anterior: {t}");
        }
    }

    // Si el release ya existe (ej. un intento parcial), igual subimos el asset.
    run_ok("gh", &["release", "create", &tag, "--repo", repo, "--title", &tag,
        "--notes", &format!("Release de {}", r.package.name)], None);
    run("gh", &["release", "upload", &tag, "--repo", repo, "--clobber", &tarball], None);
    let _ = fs::remove_dir_all(format!("{}/tmp", recipe::FIRECIPES_DIR));
    println!("Publicado {tag} en {repo}");
}