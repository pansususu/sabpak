mod builder;
mod config;
mod helper;
mod install;
mod recipe;
mod remove;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("install") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea instalar?");
                return;
            };
            install::install_package(pkg);
        }
        Some("remove") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea remover?");
                return;
            };
            remove::remove_package(pkg);
        }
        Some("build") => {
            let Some(pkg) = args.get(2) else {
                println!("¿Que paquete desea compilar?");
                return;
            };
            builder::build_package(pkg);
        }
        Some("new") => {
            let Some(receta) = args.get(2) else {
                println!("¿Que receta desea crear?");
                return;
            };
            helper::new_recipe(receta);
        }
        Some("search") => println!("elun search: en progreso"),
        Some("version") | Some("--version") => println!("elun 0.1"),
        Some("help") | None => {
            println!("elun install <paquete>: Instala un paquete a tu sistema");
            println!("elun remove <paquete>: Remueve un paquete de tu sistema");
            println!("elun search <paquete>: Buscar un paquete en los repositorios");
            println!("elun new <receta>: Crea una nueva receta con el helper");
            println!("elun build <receta>: Compila, empaqueta y publica una receta");
            println!("elun version: Muestra la version");
        }
        Some(_) => println!("Comando desconocido"),
    }
}
