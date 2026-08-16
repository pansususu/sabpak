#!/usr/bin/env bash
#
# Genera la variante "elun" (para el amigo) a partir del sabpak actual y la
# publica en el repo de Elun. NO toca tu base: trabaja en una copia temporal.
#
# Uso:  scripts/make-elun.sh [rama]
#   rama  (opcional) rama destino en el repo del amigo. Default: elun
#
set -euo pipefail

AMIGO_REPO="${AMIGO_REPO:-https://github.com/Mauneth/elun}"
BRANCH="${1:-elun}"
SRC="$(cd "$(dirname "$0")/.." && pwd)"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# 1) Copia la base, sin basura de git/build ni recetas de prueba.
cp -a "$SRC/." "$STAGE/"
rm -rf "$STAGE/.git" "$STAGE/target" "$STAGE/firecipes" "$STAGE/recipes/ripgrep.toml"

# 2) Re-renombra a elun en el código y en los scripts (sabpak -> elun,
#    SABPAK_PREFIX -> ELUN_PREFIX) para que la variante sea autosuficiente.
find "$STAGE" -type f \
  \( -name '*.rs' -o -name 'Cargo.toml' -o -name 'Cargo.lock' -o -name '*.sh' \) \
  -exec sed -i 's/SABPAK_PREFIX/ELUN_PREFIX/g; s/ELUN_PREFIX/ELUN_PREFIX/g; s/sabpak/elun/g' {} +
# El sed anterior renombró también scripts/install-crux.sh dentro del stage.
find "$STAGE"/scripts -maxdepth 1 -type f -iname '*sabpak*' -exec sh -c '
  for f; do mv "$f" "$(dirname "$f")/$(basename "$f" | sed s/sabpak/elun/g)"; done
' sh {} + || true

# 3) Commit + push a la rama destino del repo del amigo.
cd "$STAGE"
git init -q
git add -A
git -c user.name="$(git config user.name || echo elun)" \
    -c user.email="$(git config user.email || echo elun@local)" \
    commit -q -m "elun: gestor de paquetes (generado de sabpak)"
echo "Publicando $BRANCH en $AMIGO_REPO"
git push -f "$AMIGO_REPO" HEAD:refs/heads/"$BRANCH"
echo "OK -> $AMIGO_REPO (rama $BRANCH)"