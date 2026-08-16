use crate::{config, recipe};
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::Command;

fn dist() -> String {
    format!("https://github.com/{}/releases/download", config::releases_repo())
}

fn download(r: &recipe::Recipe) -> Option<PathBuf> {
    let dir = config::user_cache();
    fs::create_dir_all(&dir).ok();
    let fname = recipe::tarball_name(r);
    let tarball = dir.join(&fname);
    // Reutilizamos la caché solo si el tarball no está vacío ni enviado a
    // medias (devuelto por un rename previo antes de terminar de escribir).
    if tarball.exists() && fs::metadata(&tarball).map(|m| m.len() > 0).unwrap_or(false) {
        return Some(tarball);
    }
    let url = format!("{}/{}/{}", dist(), recipe::release_tag(r), fname);
    let resp = match ureq::get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => {
            eprintln!("No existe el release {fname}");
            return None;
        }
        Err(ureq::Error::Status(code, _)) => {
            eprintln!("Descarga de {fname} falló (HTTP {code})");
            return None;
        }
        Err(e) => {
            eprintln!("No se pudo descargar {fname}: {e}");
            return None;
        }
    };
    // Parte temporal única por proceso para evitar que dos procesos se pisen.
    let part = dir.join(format!("{fname}.part.{}", std::process::id()));
    let res = (|| -> std::io::Result<()> {
        let mut out = BufWriter::with_capacity(1 << 16, fs::File::create(&part)?);
        let mut src = BufReader::with_capacity(1 << 16, resp.into_reader());
        std::io::copy(&mut src, &mut out)?;
        out.flush()?;
        fs::rename(&part, &tarball)?;
        Ok(())
    })();
    if let Err(e) = res {
        let _ = fs::remove_file(&part);
        eprintln!("No se pudo guardar {fname}: {e}");
        return None;
    }
    Some(tarball)
}

/// Instala `nombre` y todas sus dependencias en orden. Un error en un
/// paquete no aborta el resto (devuelve cuántos fallaron).
pub fn install_package(nombre: &str) -> usize {
    let mut fails = 0;
    for p in recipe::resolve(nombre) {
        if config::bin_name(&p).is_some() {
            println!("{p} ya está instalado");
            continue;
        }
        if !install_one(&p, None) {
            fails += 1;
        }
    }
    fails
}

/// Instala `nombre`; si `version` es `Some`, fuerza esa versión. Solo
/// resuelve ese paquete (sin sus dependencias), útil para `update`.
pub fn install_version(nombre: &str, version: Option<String>) -> bool {
    install_one(nombre, version)
}

fn install_one(nombre: &str, version: Option<String>) -> bool {
    let mut r = recipe::load(nombre);
    if let Some(v) = version {
        r.package.version = v;
    }
    let tarball = match download(&r) {
        Some(t) => t,
        None => return false,
    };

    let staged = config::user_cache().join("stow");
    let _ = fs::remove_dir_all(&staged);
    fs::create_dir_all(&staged).ok();
    if !Command::new("tar")
        .args(["xzf"]).arg(&tarball).arg("-C").arg(&staged)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        eprintln!("Fallo al extraer {tarball:?}");
        return false;
    }

    let Some(out) = recipe::find_binary(&staged.to_string_lossy(), &r.build.output, &r.package.name)
    else {
        eprintln!("No se encontró binario de '{nombre}' dentro del paquete");
        let _ = fs::remove_dir_all(&staged);
        return false;
    };
    let src = staged.join(&out);
    let name = out.rsplit('/').next().unwrap_or(&r.package.name);
    let bin = config::bin_dir().join(name);
    if !config::run_elev("install", &["-D", "-m", "0755",
        src.to_str().unwrap(), bin.to_str().unwrap()])
    {
        eprintln!("No se pudo instalar {nombre} en {}", bin.display());
        let _ = fs::remove_dir_all(&staged);
        return false;
    }
    let _ = fs::remove_dir_all(&staged);
    config::remember(nombre, name, &r.package.version);
    println!("Instalado {nombre} v{} -> {}", r.package.version, bin.display());
    true
}