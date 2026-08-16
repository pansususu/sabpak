#!/usr/bin/env bash
#
# install-crux.sh — Instalación de CRUX + gestor de paquetes sabpak.
# Se ejecuta desde la ISO de CRUX (live). No asume un disco fijo:
# si DISK no está definido, se elige interactivamente.
#
# Uso:
#   DISK=/dev/sda ./install-crux.sh
#   IMAGEN=/media/rootfs.tar.xz ./install-crux.sh   # base de CRUX (auto-detectada)
#
set -euo pipefail

REPO="${REPO:-https://github.com/pansususu/sabpak}"
DISK="${DISK:-}"
USERNAME="${USERNAME:-sabrina}"
HOSTNAME="${HOSTNAME:-finix}"
MOUNT=/mnt

# --- Elegir disco (no se asume la máquina) ---
if [ -z "$DISK" ]; then
    echo "Discos disponibles:"
    lsblk -d -o NAME,SIZE,MODEL | sed 1d
    read -rp "Escribe el disco (por ejemplo: sda o nvme0n1, sin /dev/): " disk
    DISK="/dev/$disk"
fi
[ -b "$DISK" ] || { echo "No es un bloque válido: $DISK"; exit 1; }

ESP_SIZE="${ESP_SIZE:-1G}"
SWAP_SIZE="${SWAP_SIZE:-16G}"
ROOT_SIZE="${ROOT_SIZE:-70G}"
MIN_HOME="${MIN_HOME:-2G}"          # /home mínimo a reservar

echo "Target: $DISK"
echo "Layout:"
echo "  ${DISK}p1 -> EFI    ${ESP_SIZE}"
echo "  ${DISK}p2 -> SWAP   ${SWAP_SIZE}"
echo "  ${DISK}p3 -> /      ${ROOT_SIZE}"
echo "  ${DISK}p4 -> /home  (resto)"
echo ""
read -rp 'Escribe "yes" para continuar: ' answer
[ "$answer" = "yes" ] || exit 1

# --- Contraseñas ---
read -rsp 'Contraseña de root: ' root_pass; echo
read -rsp 'Confirmar root: ' root_pass2; echo
[ "$root_pass" = "$root_pass2" ] || { echo "Las contraseñas no coinciden"; exit 1; }

read -rsp "Contraseña para $USERNAME: " user_pass; echo
read -rsp "Confirmar $USERNAME: " user_pass2; echo
[ "$user_pass" = "$user_pass2" ] || { echo "Las contraseñas no coinciden"; exit 1; }

set -x

to_mib() {
    local v=$1 n k
    case "$v" in
        *[Gg]) n=${v%[Gg]}; k=1024 ;;
        *[Mm]) n=${v%[Mm]}; k=1 ;;
        *) n=$v; k=1 ;;
    esac
    echo $((n * k))
}

# Tamaño del disco en MiB (con guarda por si ROOT_SIZE no cabe).
DISK_M=$(( $(blockdev --getsize64 "$DISK" 2>/dev/null || lsblk -bno SIZE "$DISK") / 1048576 ))
ESP=$(to_mib "$ESP_SIZE"); SWAP=$(to_mib "$SWAP_SIZE"); ROOT=$(to_mib "$ROOT_SIZE")
if [ "$ROOT" -gt $(( DISK_M - (ESP + SWAP + $(to_mib "$MIN_HOME")) )) ]; then
    ROOT=$(( DISK_M - (ESP + SWAP + $(to_mib "$MIN_HOME")) ))
fi
E1=$((1 + ESP)); S1=$((E1 + SWAP)); R1=$((S1 + ROOT))

# --- Particionado (GPT) ---
parted -s "$DISK" -- mklabel gpt \
    mkpart primary fat32 1MiB ${E1}MiB \
    mkpart primary linux-swap ${E1}MiB ${S1}MiB \
    mkpart primary ext4 ${S1}MiB ${R1}MiB \
    mkpart primary ext4 ${R1}MiB 100% \
    set 1 esp on
sleep 2
partprobe "$DISK" || true

P1="${DISK}p1"; P2="${DISK}p2"; P3="${DISK}p3"; P4="${DISK}p4"

# --- Formateo ---
mkfs.fat -F 32 -n CRUXBOOT "$P1"
mkswap -L CRUXSWAP "$P2"
mkfs.ext4 -L CRUXROOT "$P3"
mkfs.ext4 -L CRUXHOME "$P4"

# --- Montado ---
mount "$P3" "$MOUNT"
mount --mkdir "$P1" "$MOUNT/boot"
mount --mkdir "$P4" "$MOUNT/home"
swapon "$P2"

# --- Extraer base de CRUX ---
if [ -z "${IMAGEN:-}" ]; then
    # En el live, la ISO suele estar en /media con el rootfs base.
    IMAGEN="$(find /media / -maxdepth 3 -type f \( -iname 'rootfs.tar.xz' -o -iname 'crux-*.tar.xz' \) 2>/dev/null | head -1)"
fi
if [ -n "$IMAGEN" ] && [ -f "$IMAGEN" ]; then
    tar -xJf "$IMAGEN" -C "$MOUNT"
else
    echo "No encontré la imagen base. Réplicala a $MOUNT manualmente y vuelve a ejecutar desde 'chroot'."
    exit 1
fi

# --- Montajes para el chroot ---
mount --mkdir --bind /proc "$MOUNT/proc"
mount --mkdir --bind /sys  "$MOUNT/sys"
mount --mkdir --bind /dev  "$MOUNT/dev"
mount --make-rslave "$MOUNT/proc"
mount --make-rslave "$MOUNT/sys"
mount --make-rslave "$MOUNT/dev"
[ -d "$MOUNT/dev/pts" ] && mount --bind /dev/pts "$MOUNT/dev/pts" || true
cp /etc/resolv.conf "$MOUNT/etc/resolv.conf" 2>/dev/null || true

chrun() { chroot "$MOUNT" /bin/bash -lc "$1"; }

# --- Configuración base ---
{
    echo "# <device>  <mountpoint>  <type>  <options>  <dump>  <pass>"
    echo "UUID=$(blkid -s UUID -o value "$P3")  /          ext4   rw,relatime  0 1"
    echo "UUID=$(blkid -s UUID -o value "$P1")  /boot      vfat   rw,relatime,fmask=0022,dmask=0022  0 2"
    echo "UUID=$(blkid -s UUID -o value "$P4")  /home      ext4   rw,relatime  0 2"
    echo "UUID=$(blkid -s UUID -o value "$P2")  swap       swap   defaults    0 0"
} > "$MOUNT/etc/fstab"

echo "$HOSTNAME" > "$MOUNT/etc/hostname"
chrun "ln -sf /usr/share/zoneinfo/UTC /etc/localtime"

# --- Usuario y contraseñas (shadow/useradd son core en CRUX ≥3.5) ---
chrun "echo 'root:$root_pass' | chpasswd"
if ! chrun "id -u $USERNAME"; then
    chrun "useradd -m -s /bin/bash -G wheel $USERNAME"
fi
chrun "echo '$USERNAME:$user_pass' | chpasswd"

# --- Toolchain: git, sudo, gh y rust ---
# En CRUX real con ports: prt-get update && prt-get install git sudo github-cli rust.
# Como alternativa rápida (requiere curl/ca-certificates en la base) se usa rustup.
chrun "export CARGO_HOME=/root/.cargo RUSTUP_HOME=/root/.rustup; \\
       command -v cargo >/dev/null 2>&1 || \\
       ( command -v curl >/dev/null 2>&1 && \\
         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal ) || \\
         echo 'AVISO: instala rust con: prt-get update && prt-get install rust'"
# `sudo` y `gh` son imprescindibles (el prefijo por defecto es de root y las
# publicaciones usan gh). Intento instalarlos por ports; si no, aviso claro.
chrun "command -v sudo >/dev/null 2>&1 || (prt-get update >/dev/null 2>&1 && prt-get install sudo) || \\
         echo 'AVISO: instala sudo con: prt-get install sudo'"
chrun "command -v gh >/dev/null 2>&1 || (prt-get update >/dev/null 2>&1 && prt-get install github-cli) || \\
         echo 'AVISO: instala gh (github-cli) con: prt-get install github-cli'"

# --- Repo y gestor sabpak ---
if ! chrun "command -v git >/dev/null 2>&1"; then
    echo "AVISO: falta git. Instálalo dentro con: prt-get install git, luego corre la parte final del script."
else
    chrun "export PATH=/root/.cargo/bin:\$PATH; \\
           git clone --depth 1 $REPO /usr/local/src/sabpak 2>/dev/null || true; \\
           cd /usr/local/src/sabpak && \\
           cargo build --release && \\
           install -Dm755 target/release/sabpak /usr/local/bin/sabpak"
fi
# SABPAK_DIR para que las recetas se resuelvan desde el árbol clonado.
mkdir -p "$MOUNT/usr/local/src/sabpak/recipes" "$MOUNT/usr/local/src/sabpak/firecipes"
mkdir -p "$MOUNT/etc/profile.d"
cat > "$MOUNT/etc/profile.d/zz-sabpak.sh" <<'EOF'
SABPAK_DIR=/usr/local/src/sabpak
export SABPAK_DIR
EOF

# --- Salida ---
swapon -a 2>/dev/null || true
echo ""
echo "Listo. Reinicia con: reboot"
echo "Luego loguea como $USERNAME y ejecuta 'sabpak' para verificar."
echo "Nota: el gestor baja releases de $REPO; ajusta SABPAK_PREFIX/SABPAK_DIR si instalas en otro sitio."