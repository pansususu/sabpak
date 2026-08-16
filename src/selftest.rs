use crate::recipe::{self, Build, Package, Recipe, Source};
use crate::ver;
use std::cmp::Ordering;

/// Ejecuta la suite de autorevisión. Devuelve `(ok, fallidas)`.
/// `build` exige que `ok >= 30` y `fallidas == 0` antes de compilar.
pub fn run() -> (u32, u32) {
    let mut ok = 0u32;
    let mut fail = 0u32;
    let mut t = |cond: bool, msg: &str| {
        if cond {
            ok += 1;
        } else {
            fail += 1;
            eprintln!("  [FALLO] {msg}");
        }
    };

    // --- Nombrado de artefactos (recipe) ---
    let r = |name: &str, version: &str| Recipe {
        package: Package { name: name.to_string(), version: version.to_string(), deps: Vec::new() },
        source: Source { kind: "git".to_string(), url: String::new(), tag: String::new() },
        build: Build { kind: "make".to_string(), args: Vec::new(), output: String::new() },
    };
    t(recipe::tarball_name(&r("rg", "2.0")) == "rg-2.0.tar.gz", "tarball_name básico");
    t(recipe::tarball_name(&r("ripgrep", "14.1.1")) == "ripgrep-14.1.1.tar.gz", "tarball_name multi-dígito");
    t(
        recipe::tarball_name(&r("tool", "1.0.0-beta1")) == "tool-1.0.0-beta1.tar.gz",
        "tarball_name con pre-release",
    );
    t(recipe::release_tag(&r("rg", "2.0")) == "rg-v2.0", "release_tag básico");
    t(
        recipe::release_tag(&r("my-tool", "0.3.2")) == "my-tool-v0.3.2",
        "release_tag con guiones",
    );

    // --- Comparación de versiones (ver) ---
    t(ver::cmp("1.0", "1.0") == Ordering::Equal, "iguales 1.0 == 1.0");
    t(ver::cmp("1.0.0", "1.0.1") == Ordering::Less, "1.0.0 < 1.0.1");
    t(ver::cmp("1.0.1", "1.0.0") == Ordering::Greater, "1.0.1 > 1.0.0");
    t(ver::cmp("1.2", "1.10") == Ordering::Less, "1.2 < 1.10 (numérico)");
    t(ver::cmp("1.10", "1.2") == Ordering::Greater, "1.10 > 1.2");
    t(ver::cmp("2.0", "1.9") == Ordering::Greater, "2.0 > 1.9");
    t(ver::cmp("10", "9") == Ordering::Greater, "10 > 9");
    t(ver::cmp("0.9", "1.0") == Ordering::Less, "0.9 < 1.0");
    t(ver::cmp("3.14.15", "3.14.9") == Ordering::Greater, "3.14.15 > 3.14.9");
    t(ver::cmp("3.14.9", "3.14.15") == Ordering::Less, "3.14.9 < 3.14.15");
    t(ver::cmp("1.2.3", "1.2") == Ordering::Greater, "1.2.3 > 1.2");
    t(ver::cmp("1.2", "1.2.3") == Ordering::Less, "1.2 < 1.2.3");
    t(ver::cmp("1.0.0.0", "1.0.0") == Ordering::Equal, "1.0.0.0 == 1.0.0");
    t(ver::cmp("5.5", "5.5") == Ordering::Equal, "5.5 == 5.5");
    t(ver::cmp("0.0.1", "0.0.2") == Ordering::Less, "0.0.1 < 0.0.2");
    t(ver::cmp("1", "1.0.0") == Ordering::Equal, "1 == 1.0.0");
    t(ver::cmp("2.2.2", "2.2.1") == Ordering::Greater, "2.2.2 > 2.2.1");
    t(ver::cmp("0", "1") == Ordering::Less, "0 < 1");
    t(ver::cmp("100", "99") == Ordering::Greater, "100 > 99");
    t(ver::cmp("1.0.0", "") == Ordering::Greater, "1.0.0 > (vacío)");

    // --- Detección de build system (builder) ---
    for (got, want) in detect_build_cases() {
        t(got == want, &format!("detectar build: se esperaba '{want}', se obtuvo '{got}'"));
    }

    (ok, fail)
}

/// Prueba `builder::detect_build` sobre directorios temporales reales.
fn detect_build_cases() -> Vec<(String, String)> {
    let cases = [
        ("Cargo.toml", "cargo"),
        ("CMakeLists.txt", "cmake"),
        ("Makefile", "make"),
        ("makefile", "make"),
        ("GNUmakefile", "make"),
    ];
    let mut out = Vec::new();
    for (file, want) in cases.iter() {
        let dir = std::env::temp_dir().join(format!("sabpak-detect-{}-{}", *file, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(file), "x\n");
        let (kind, _) = crate::builder::detect_build(&dir.to_string_lossy());
        out.push((kind.to_string(), want.to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn suite_pasa_30() {
        let (ok, fail) = run();
        assert_eq!(fail, 0, "hay aserciones fallidas en la autorevisión");
        assert!(ok >= 30, "la suite debería tener ≥30 tests, hay {ok}");
    }
}