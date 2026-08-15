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

/// Recuerda qué binario instaló un paquete (el nombre instalado puede
/// diferir del paquete, p.ej. `rg` vs `ripgrep`).
pub fn remember(name: &str, bin: &str) {
    let keep = entries()
        .into_iter()
        .filter(|l| !l.starts_with(&format!("{name} ")))
        .collect::<Vec<_>>()
        .join("\n");
    let text = if keep.is_empty() {
        format!("{name} {bin}\n")
    } else {
        format!("{keep}\n{name} {bin}\n")
    };
    write_state(&text);
}

/// Binario instalado para `name`, si se registró.
pub fn bin_name(name: &str) -> Option<String> {
    entries().into_iter().find_map(|l| {
        let mut it = l.split(' ');
        match (it.next(), it.next()) {
            (Some(p), Some(b)) if p == name => Some(b.to_string()),
            _ => None,
        }
    })
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
        .filter_map(|l| l.split(' ').next().map(str::to_string))
        .collect()
}

/// Borra caché huérfana/incompleta, entradas de estado de binarios ya
/// inexistentes y temporales residuales. Devuelve cuántos elementos limpió.
pub fn cleanup() -> usize {
    let mut freed = 0;

    // 1) Caché: descargas incompletas (.part) u huérfanas (paquete no instalado).
    if let Ok(rd) = std::fs::read_dir(user_cache()) {
        let installed = installed_packages();
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
    let keep: Vec<String> = entries()
        .into_iter()
        .filter(|l| l.split(' ').next_back().map(|b| bin.join(b).exists()).unwrap_or(false))
        .collect();
    let stale = entries().len() - keep.len();
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
fn write_state(text: &str) {
    use std::io::Write;
    let file = state_file();
    let script = format!(
        "mkdir -p {} && cat > {}",
        file.parent().unwrap().display(),
        file.display()
    );
    let mut c = Command::new(if needs_sudo() { "sudo" } else { "sh" });
    if needs_sudo() {
        c.arg("sh");
    }
    let mut child = c
        .arg("-c")
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok();
    if let Some(mut ch) = child.take() {
        if let Some(stdin) = ch.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = ch.wait();
    }
}