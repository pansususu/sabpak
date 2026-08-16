use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Prefijo de instalación. Defecto sistema (`/usr/local`), como una distro
/// normal. Sobrescribible con `SABPAK_PREFIX` (útil en desarrollo o para el
/// amigo que quiera otro lugar).
pub fn prefix() -> PathBuf {
    match std::env::var("SABPAK_PREFIX") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from("/usr/local"),
    }
}

/// Directorio base para recetas y firecipes. Defecto: prefijo/share/sabpak.
/// Sobrescribible con `SABPAK_DIR` (dev, o donde instalaste el árbol de
/// recetas, p.ej. /usr/local/src/sabpak).
pub fn base_dir() -> PathBuf {
    match std::env::var("SABPAK_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => prefix().join("share/sabpak"),
    }
}

/// true si el prefijo no es escribible por el usuario (hay que elevar con sudo).
pub fn needs_sudo() -> bool {
    *NEEDS_SUDO.get_or_init(probe_sudo)
}

fn probe_sudo() -> bool {
    let dir = bin_dir();
    std::fs::create_dir_all(&dir).ok();
    let probe = dir.join(".sabpak_probe");
    let writable = std::fs::write(&probe, b"x").is_ok();
    std::fs::remove_file(&probe).ok();
    !writable
}

static NEEDS_SUDO: OnceLock<bool> = OnceLock::new();

/// Caché por-usuario (siempre escribible sin sudo).
pub fn user_cache() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".cache/sabpak")
}

pub fn bin_dir() -> PathBuf {
    prefix().join("bin")
}

fn state_file() -> PathBuf {
    prefix().join("share/sabpak").join("installed.txt")
}

/// Repo donde se publican los binarios. Configurable para cuando el amigo
/// monte su servidor (`SABPAK_RELEASES` / `ELUN_RELEASES`).
pub fn releases_repo() -> String {
    match std::env::var("SABPAK_RELEASES") {
        Ok(r) if !r.is_empty() => r,
        _ => "pansususu/packages".to_string(),
    }
}

/// Ejecuta un comando elevado con sudo si hace falta.
pub fn run_elev(cmd: &str, args: &[&str]) -> bool {
    let status = match needs_sudo() {
        true => Command::new("sudo").arg(cmd).args(args).status(),
        false => Command::new(cmd).args(args).status(),
    };
    match status {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("'{cmd}' falló con código {}", s.code().unwrap_or(-1));
            false
        }
        Err(e) => {
            eprintln!("No se pudo ejecutar '{cmd}': {e}");
            false
        }
    }
}

fn state_lines() -> String {
    std::fs::read_to_string(state_file()).unwrap_or_default()
}

/// Entradas no vacías del estado (cada una `paquete binario`).
fn entries() -> Vec<String> {
    state_lines()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Recuerda qué binario instaló un paquete, con su versión. El nombre
/// instalado puede diferir del paquete (p.ej. `rg` vs `ripgrep`).
pub fn remember(name: &str, bin: &str, version: &str) {
    let keep = entries()
        .into_iter()
        .filter(|l| !l.starts_with(&format!("{name} ")))
        .collect::<Vec<_>>()
        .join("\n");
    let line = format!("{name} {bin} {version}");
    let text = if keep.is_empty() {
        format!("{line}\n")
    } else {
        format!("{keep}\n{line}\n")
    };
    write_state(&text);
}

/// Binario instalado para `name`, si se registró.
pub fn bin_name(name: &str) -> Option<String> {
    entries().into_iter().find_map(|l| {
        let v: Vec<&str> = l.split(' ').collect();
        (v.first() == Some(&name)).then(|| v.get(1).unwrap_or(&"").to_string())
    })
}

/// Versión instalada para `name`, si se registró.
pub fn installed_version(name: &str) -> Option<String> {
    entries().into_iter().find_map(|l| {
        let v: Vec<&str> = l.split(' ').collect();
        (v.first() == Some(&name)).then(|| v.get(2).unwrap_or(&"").to_string())
    })
}

/// Paquetes instalados: `(paquete, binario, versión)`.
pub fn installed() -> Vec<(String, String, String)> {
    entries()
        .into_iter()
        .filter_map(|l| {
            let v: Vec<&str> = l.split(' ').collect();
            match (v.get(0), v.get(1), v.get(2)) {
                (Some(n), Some(b), Some(ver)) => {
                    Some((n.to_string(), b.to_string(), ver.to_string()))
                }
                _ => None,
            }
        })
        .collect()
}

/// Olvida el registro de instalación de `name`.
pub fn forget(name: &str) {
    let keep = entries()
        .into_iter()
        .filter(|l| !l.starts_with(&format!("{name} ")))
        .collect::<Vec<_>>();
    if keep.is_empty() {
        write_state("");
    } else {
        write_state(&format!("{}\n", keep.join("\n")));
    }
}

/// Paquetes instalados según el estado.
pub fn installed_packages() -> Vec<String> {
    entries()
        .into_iter()
        .filter_map(|l| l.split(' ').next().map(str::to_string).filter(|s| !s.is_empty()))
        .collect()
}

/// Borra caché huérfana/incompleta, entradas de estado de binarios ya
/// inexistentes y temporales residuales. Devuelve cuántos elementos limpió.
pub fn cleanup() -> usize {
    let mut freed = 0;

    // 1) Caché: descargas incompletas (.part) u huérfanas (paquete no instalado).
    let installed = installed_packages();
    if let Ok(rd) = std::fs::read_dir(user_cache()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".part")
                || !installed.iter().any(|p| name.starts_with(&format!("{p}-")))
            {
                if std::fs::remove_file(e.path()).is_ok() {
                    freed += 1;
                }
            }
        }
    }

    // 2) Temporales residuales de la extracción.
    if std::fs::remove_dir_all(user_cache().join("stow")).is_ok() {
        freed += 1;
    }

    // 3) Entradas de estado de binarios que ya no existen.
    let bin = bin_dir();
    let all = entries();
    let keep: Vec<String> = all
        .iter()
        .filter(|l| l.split(' ').nth(1).map(|b| bin.join(b).exists()).unwrap_or(false))
        .cloned()
        .collect();
    let stale = all.len() - keep.len();
    if stale > 0 {
        freed += stale;
        let text = if keep.is_empty() {
            String::new()
        } else {
            format!("{}\n", keep.join("\n"))
        };
        write_state(&text);
    }
    freed
}

/// Escribe el fichero de estado (con sudo si el prefijo es de sistema).
/// Todo por argumentos separados (nada de cadenas shell ensambladas).
fn write_state(text: &str) {
    use std::io::Write;
    let file = state_file();
    let parent = file.parent().expect("state file sin directorio padre");
    let (dir_s, file_s) = (parent.to_str().unwrap(), file.to_str().unwrap());
    if needs_sudo() {
        if !run_elev("install", &["-d", dir_s]) {
            return;
        }
        if let Ok(mut child) = Command::new("sudo")
            .args(["tee", file_s])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    } else {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let _ = std::fs::write(&file, text);
    }
}