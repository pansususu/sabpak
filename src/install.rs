use crate::{config, recipe};
use std::fs;
use std::io::copy;
use std::path::PathBuf;
use std::process::Command;

fn dist() -> String {
    format!("https://github.com/{}/releases/download", recipe::RELEASES_REPO)
}

fn download(r: &recipe::Recipe) -> PathBuf {
    let dir = config::user_cache();
    fs::create_dir_all(&dir).ok();
    let fname = recipe::tarball_name(r);
    let tarball = dir.join(&fname);
    if tarball.exists() {
        return tarball;
    }
    let resp = ureq::get(&format!("{}/{}/{}", dist(), recipe::release_tag(r), fname))
        .call()
        .unwrap_or_else(|e| {
            eprintln!("No se pudo descargar {fname}: {e}");
            std::process::exit(1);
        });
    let part = dir.join(format!("{fname}.part"));
    let res = (|| -> std::io::Result<()> {
        let mut out = fs::File::create(&part)?;
        copy(&mut resp.into_reader(), &mut out)?;
        fs::rename(&part, &tarball)?;
        Ok(())
    })();
    if let Err(e) = res {
        eprintln!("No se pudo guardar {fname}: {e}");
        std::process::exit(1);
    }
    tarball
}

pub fn install_package(nombre: &str) {
    let r = recipe::load(nombre);
    let tarball = download(&r);

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
        std::process::exit(1);
    }

    let out = recipe::find_binary(&staged.to_string_lossy(), &r.build.output, &r.package.name);
    let src = staged.join(&out);
    let name = out.rsplit('/').next().unwrap_or(&r.package.name);
    let bin = config::bin_dir().join(name);
    if !config::run_elev("install", &["-D", "-m", "0755",
        src.to_str().unwrap(), bin.to_str().unwrap()])
    {
        eprintln!("No se pudo instalar {nombre} en {}", bin.display());
        std::process::exit(1);
    }
    let _ = fs::remove_dir_all(&staged);
    config::remember(nombre, name);
    println!("Instalado {nombre} -> {}", bin.display());
}