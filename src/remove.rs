use crate::config;

/// Remueve `nombre`. Devuelve `bool` de éxito para no abortar lotes.
pub fn remove_package(nombre: &str) -> bool {
    let bin = config::bin_name(nombre).unwrap_or_else(|| nombre.to_string());
    let path = config::bin_dir().join(&bin);
    if !path.exists() {
        println!("{nombre} no está instalado");
        config::forget(nombre);
        config::cleanup();
        return false;
    }
    if config::run_elev("rm", &["-f", path.to_str().unwrap()]) {
        println!("Removido {nombre} de {}", path.display());
    } else {
        eprintln!("No se pudo remover {nombre}");
        return false;
    }
    config::forget(nombre);
    let freed = config::cleanup();
    println!("Limpieza: {freed} elemento(s) liberados");
    true
}