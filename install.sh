#!/usr/bin/env sh

set -eu

REPO="${H5V_REPO:-DanielHauge/h5v}"
INSTALL_DIR="${H5V_INSTALL_DIR:-$HOME/.local/bin}"
VERSION=""
DRY_RUN=0

usage() {
    cat <<'EOF'
Install h5v from GitHub Releases.

Usage:
  install.sh [--version VERSION] [--repo OWNER/REPO] [--install-dir PATH] [--dry-run]

Environment:
  H5V_REPO         Override the GitHub repository (default: DanielHauge/h5v)
  H5V_INSTALL_DIR  Override the install directory (default: ~/.local/bin)
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:?missing value for --version}"
            shift 2
            ;;
        --repo)
            REPO="${2:?missing value for --repo}"
            shift 2
            ;;
        --install-dir)
            INSTALL_DIR="${2:?missing value for --install-dir}"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

normalize_version() {
    case "$1" in
        v*) printf '%s\n' "${1#v}" ;;
        *) printf '%s\n' "$1" ;;
    esac
}

latest_tag() {
    curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" \
        | sed 's|.*/||'
}

verify_sha256() {
    file="$1"
    checksum_file="$2"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$(dirname "$file")" && sha256sum -c "$(basename "$checksum_file")")
    elif command -v shasum >/dev/null 2>&1; then
        expected="$(cut -d ' ' -f1 "$checksum_file")"
        actual="$(shasum -a 256 "$file" | cut -d ' ' -f1)"
        [ "$expected" = "$actual" ] || {
            echo "SHA256 mismatch for $(basename "$file")" >&2
            exit 1
        }
    else
        echo "Need sha256sum or shasum to verify the download" >&2
        exit 1
    fi
}

require_cmd curl
require_cmd tar
require_cmd mktemp
require_cmd sed

os="$(uname -s)"
arch="$(uname -m)"

case "${os}:${arch}" in
    Linux:x86_64|Linux:amd64)
        target="x86_64-unknown-linux-gnu"
        ;;
    Darwin:x86_64)
        target="x86_64-apple-darwin"
        ;;
    Darwin:arm64|Darwin:aarch64)
        target="aarch64-apple-darwin"
        ;;
    Linux:arm64|Linux:aarch64)
        echo "Linux ARM64 installers are not published yet." >&2
        exit 1
        ;;
    *)
        echo "Unsupported platform: ${os} ${arch}" >&2
        exit 1
        ;;
esac

if [ -n "$VERSION" ]; then
    version="$(normalize_version "$VERSION")"
    tag="v${version}"
else
    tag="$(latest_tag)"
    version="$(normalize_version "$tag")"
fi

archive="h5v-${target}-v${version}.tar.gz"
checksum="${archive}.sha256"
archive_url="https://github.com/${REPO}/releases/download/${tag}/${archive}"
checksum_url="https://github.com/${REPO}/releases/download/${tag}/${checksum}"

if [ "$DRY_RUN" -eq 1 ]; then
    printf 'Repository: %s\nVersion: %s\nTarget: %s\nInstall dir: %s\nArchive URL: %s\n' \
        "$REPO" "$version" "$target" "$INSTALL_DIR" "$archive_url"
    exit 0
fi

tmpdir="$(mktemp -d)"
cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup EXIT INT HUP TERM

archive_path="${tmpdir}/${archive}"
checksum_path="${tmpdir}/${checksum}"

curl -fsSL "$archive_url" -o "$archive_path"
curl -fsSL "$checksum_url" -o "$checksum_path"
verify_sha256 "$archive_path" "$checksum_path"
tar -xzf "$archive_path" -C "$tmpdir"

mkdir -p "$INSTALL_DIR"
install -m 755 "${tmpdir}/h5v-${target}-v${version}/h5v" "${INSTALL_DIR}/h5v"

printf 'Installed h5v to %s/h5v\n' "$INSTALL_DIR"
case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        printf 'Note: %s is not currently on PATH.\n' "$INSTALL_DIR" >&2
        ;;
esac
