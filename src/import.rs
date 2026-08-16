use crate::recipe;

struct Pkgfile {
    name: String,
    version: String,
    description: String,
    sources: Vec<String>,
    deps: Vec<String>,
}

/// Extrae las URLs de un trozo de la línea `source=(...)`.
fn extract_urls(chunk: &str, vars: &[(&str, &str)]) -> Vec<String> {
    chunk
        .split_whitespace()
        .map(|t| t.trim().trim_matches('"').trim_matches('\''))
        .filter(|t| t.contains("://"))
        .map(|t| {
            // Sustituye $name/$version/$release como haría el shell.
            let mut s = t.to_string();
            for (k, v) in vars {
                s = s.replace(&format!("${k}"), v).replace(&format!("${{{k}}}"), v);
            }
            s
        })
        .collect()
}

/// Parsea un `Pkgfile` de CRUX (script shell) sin evaluarlo: primera pasada
/// para las asignaciones escalares (`name`, `version`, ...), segunda para el
/// array `source=(...)` (multilínea) y el comentario `# Depends on:`.
fn parse_pkgfile(content: &str) -> Pkgfile {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();

    let mut scalars: std::collections::HashMap<String, String> = Default::default();
    for &line in &lines {
        if line.starts_with("source=(") || line.starts_with('#') {
            continue;
        }
        if let Some(i) = line.find('=') {
            let key = line[..i].trim().to_string();
            let val = line[i + 1..].trim().trim_matches('"').trim_matches('\'').to_string();
            scalars.entry(key).or_insert(val);
        }
    }
    let get = |k: &str| scalars.get(k).cloned().unwrap_or_default();

    let mut pf = Pkgfile {
        name: get("name"),
        version: get("version"),
        description: get("description"),
        sources: Vec::new(),
        deps: Vec::new(),
    };
    if pf.name.is_empty() {
        return pf;
    }
    let vars = [
        ("name", pf.name.as_str()),
        ("version", pf.version.as_str()),
        ("release", scalars.get("release").map(String::as_str).unwrap_or("")),
    ];

    let mut in_source = false;
    for &line in &lines {
        if in_source {
            let (chunk, ends) = match line.find(')') {
                Some(i) => (&line[..i], true),
                None => (line, false),
            };
            pf.sources.extend(extract_urls(chunk, &vars));
            if ends {
                in_source = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("source=(") {
            let (chunk, ends) = match rest.find(')') {
                Some(i) => (&rest[..i], true),
                None => (rest, false),
            };
            pf.sources.extend(extract_urls(chunk, &vars));
            in_source = !ends;
            continue;
        }
        if let Some(i) = line.strip_prefix('#').and_then(|s| s.find("Depends on:")) {
            let rest = &line[1 + i + "Depends on:".len()..];
            pf.deps.extend(rest.split_whitespace().map(str::to_string));
        }
    }
    pf
}

/// Importa el port de CRUX `name` como receta, usando su tarball fuente y
/// build autodetectado.
pub fn import_pkg(name: &str) {
    let ports = std::env::var("CRUX_PORTS").unwrap_or_else(|_| "/usr/ports".to_string());
    let pkgfile = format!("{ports}/{name}/Pkgfile");
    let content = match std::fs::read_to_string(&pkgfile) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No encontré el port de CRUX '{name}': {pkgfile}");
            eprintln!("Sugerencia: exporta CRUX_PORTS con la ruta de tu árbol de ports.");
            return;
        }
    };
    let pf = parse_pkgfile(&content);
    let pname = if pf.name.is_empty() { name.to_string() } else { pf.name };
    if pf.version.is_empty() {
        eprintln!("El Pkgfile de '{pname}' no declara versión; no se puede importar.");
        return;
    }
    let Some(url) = pf.sources.first() else {
        eprintln!("El Pkgfile de '{pname}' no declara source; no se puede importar.");
        return;
    };
    let receta = recipe::Recipe {
        package: recipe::Package {
            name: pname.clone(),
            version: pf.version.clone(),
            deps: pf.deps.clone(),
        },
        source: recipe::Source {
            kind: "archive".to_string(),
            url: url.clone(),
            tag: String::new(),
        },
        build: recipe::Build {
            kind: "auto".to_string(),
            args: Vec::new(),
            output: pname.clone(),
        },
    };
    let ruta = recipe::recipes_dir().join(format!("{pname}.toml"));
    let ruta = ruta.to_string_lossy().into_owned();
    std::fs::create_dir_all(recipe::recipes_dir()).ok();
    let toml = toml::to_string(&receta).unwrap_or_else(|e| {
        eprintln!("No se pudo serializar la receta: {e}");
        std::process::exit(1)
    });
    std::fs::write(&ruta, toml).unwrap_or_else(|e| {
        eprintln!("No se pudo escribir {ruta}: {e}");
        std::process::exit(1)
    });
    println!("Importada receta de CRUX: {pname} v{}", pf.version);
    println!("  fuente: {url}");
    if !pf.description.is_empty() {
        println!("  descripción: {}", pf.description);
    }
    if !pf.deps.is_empty() {
        println!("  dependencias: {}", pf.deps.join(", "));
    }
}