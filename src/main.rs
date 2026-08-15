mod builder;
mod helper;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("install") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea instalar?");
                return;
            };
            println!("Iniciando la instalación de: {pkg}");
        }
        Some("remove") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea remover?");
                return;
            };
            println!("Iniciando la limpieza de {pkg}");
        }
        Some("build") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea compilar?");
                return;
            };
            println!("Iniciando la compilacion del paquete");
            builder::build_package(pkg);
        }
        Some("new") => {
            let Some(receta) = args.get(2) else {
                println!("¿Que receta desea crear?");
                return;
            };
            helper::new_recipe(receta);
        }
        Some("search") => println!("sabpak search: en progreso"),
        Some("version") | Some("--version") => println!("sabpak 0.1"),
        Some("help") | None => {
            println!("sabpak install <paquete>: Instala un paquete a tu sistema");
            println!("sabpak remove <paquete>: Remueve un paquete de tu sistema");
            println!("sabpak search <paquete>: Buscar un paquete en los repositorios");
            println!("sabpak new <receta>: Crea una nueva receta con el helper");
            println!("sabpak build <receta>: Compila y empaqueta una receta");
            println!("sabpak version: Muestra la version");
        }
        Some(_) => println!("Comando desconocido"),
    }
}
