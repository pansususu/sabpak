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

/// Descarga una URL arbitraria a `dest` (para fuentes tipo archive).
fn download_to(url: &str, dest: &str) {
    let resp = ureq::get(url).call().unwrap_or_else(|e| {
        eprintln!("No se pudo descargar {url}: {e}");
        std::process::exit(1);
    });
    let part = format!("{dest}.part");
    let res = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&part)?;
        std::io::copy(&mut resp.into_reader(), &mut f)?;
        fs::rename(&part, dest)?;
        Ok(())
    })();
    if let Err(e) = res {
        eprintln!("No se pudo guardar {dest}: {e}");
        std::process::exit(1);
    }
}

/// Detecta el build system de un directorio de fuentes ya extraído.
pub(crate) fn detect_build(root: &str) -> (&'static str, Vec<&'static str>) {
    let has = |p: &str| std::fs::metadata(format!("{root}/{p}")).is_ok();
    if has("Cargo.toml") {
        ("cargo", vec!["--release"])
    } else if has("CMakeLists.txt") {
        ("cmake", vec!["-DCMAKE_BUILD_TYPE=Release"])
    } else {
        ("make", vec!["-j"])
    }
}

/// Si `dir` contiene un único subdirectorio (típico de tarballs que extraen en
/// `proyecto-versión/`), devuelve ese subdirectorio; si no, devuelve `dir`.
fn archive_root(dir: &str) -> String {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    if entries.len() == 1 {
        entries[0].path().to_string_lossy().into_owned()
    } else {
        dir.to_string()
    }
}

fn run_build(workdir: &str, kind: &str, args: &[&str]) -> bool {
    match kind {
        "cargo" => {
            let mut a = vec!["build"];
            a.extend_from_slice(args);
            run_ok("cargo", &a, Some(workdir))
        }
        "make" => run_ok("make", args, Some(workdir)),
        "cmake" => {
            run_ok("cmake", &["-S", ".", "-B", "build", "-DCMAKE_BUILD_TYPE=Release"], Some(workdir))
                && run_ok("cmake", &["--build", "build", "-j"], Some(workdir))
        }
        other => {
            eprintln!("Tipo de build no soportado: '{other}'");
            false
        }
    }
}

pub fn build_package(nombre: &str) -> bool {
    let r = recipe::load(nombre);
    let fc = recipe::firecipes_dir();
    let workdir = fc.join("tmp").join(format!("{}-{}", r.package.name, r.package.version));
    fs::create_dir_all(&fc).unwrap_or_else(|e| {
        eprintln!("No se pudo crear {}: {e}", fc.display());
        std::process::exit(1);
    });
    let _ = fs::remove_dir_all(&workdir);
    fs::create_dir_all(&workdir).unwrap_or_else(|e| {
        eprintln!("No se pudo crear el directorio de trabajo {}: {e}", workdir.display());
        std::process::exit(1);
    });

    // --- Obtener fuentes ---
    let mut build_root = workdir.to_string_lossy().into_owned();
    match r.source.kind.as_str() {
        "git" => {
            run(
                "git",
                &["clone", "--depth", "1", "--branch", &r.source.tag, &r.source.url, workdir.to_str().unwrap()],
                None,
            );
        }
        "archive" => {
            let fname = r.source.url.rsplit('/').next().unwrap_or("source.tar.gz");
            let archive = fc.join(fname);
            download_to(&r.source.url, archive.to_str().unwrap());
            run("tar", &["xf", archive.to_str().unwrap(), "-C", workdir.to_str().unwrap()], None);
            // Los tarballs suelen extraer en una única subcarpeta de primer
            // nivel (`pkg-version/`); entramos en ella para compilar.
            build_root = archive_root(workdir.to_str().unwrap());
        }
        other => {
            eprintln!("Tipo de fuente no soportado: '{other}'");
            let _ = fs::remove_dir_all(&workdir);
            return false;
        }
    }

    // Si el kind es "auto" (o la receta no lo indica claro), detectamos del árbol.
    let (kind, args): (String, Vec<String>) = if r.build.kind == "auto" {
        let (k, a) = detect_build(&build_root);
        (k.to_string(), a.into_iter().map(str::to_string).collect())
    } else {
        (r.build.kind.clone(), r.build.args.clone())
    };
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let ok = run_build(&build_root, &kind, &args_ref);
    if !ok {
        let _ = fs::remove_dir_all(&workdir);
        return false;
    }

    let Some(bin_rel) = recipe::find_binary(workdir.to_str().unwrap(), &r.build.output, &r.package.name) else {
        eprintln!(
            "No se pudo autodetectar el binario de '{}' en {} (se esperaba '{}/')",
            r.package.name, workdir.display(), r.build.output
        );
        let _ = fs::remove_dir_all(&workdir);
        return false;
    };
    println!("Binario detectado: {bin_rel}");
    let tarball = fc.join(recipe::tarball_name(&r));
    run(
        "tar",
        &["czf", tarball.to_str().unwrap(), "-C", workdir.to_str().unwrap(), bin_rel.as_str()],
        None,
    );
    let _ = fs::remove_dir_all(&workdir);
    println!("Paquete listo: {}", tarball.display());

    let repo = crate::config::releases_repo();
    let tag = recipe::release_tag(&r);
    let prefix = format!("{}-v", r.package.name);
    for t in out("gh", &["release", "list", "--repo", &repo,
        "--json", "tagName", "--jq", ".[].tagName"]).lines()
    {
        let t = t.trim();
        if t != tag && t.starts_with(&prefix) {
            run("gh", &["release", "delete", t, "--repo", &repo, "--yes", "--cleanup-tag"], None);
            println!("Eliminada release anterior: {t}");
        }
    }

    // Si el release ya existe (ej. un intento parcial), igual subimos el asset.
    run_ok("gh", &["release", "create", &tag, "--repo", &repo, "--title", &tag,
        "--notes", &format!("Release de {}", r.package.name)], None);
    run("gh", &["release", "upload", &tag, "--repo", &repo, "--clobber", tarball.to_str().unwrap()], None);
    let _ = fs::remove_dir_all(fc.join("tmp"));
    println!("Publicado {tag} en {repo}");
    true
}