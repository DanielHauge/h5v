#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
from pathlib import Path
import textwrap


LINUX_X64_TARGET = "x86_64-unknown-linux-gnu"
# MACOS_X64_TARGET = "x86_64-apple-darwin"
MACOS_ARM64_TARGET = "aarch64-apple-darwin"
WINDOWS_X64_TARGET = "x86_64-pc-windows-msvc"


def asset_name(version: str, target: str, extension: str) -> str:
    return f"h5v-{target}-v{version}.{extension}"


def sha256_for(asset_dir: Path, filename: str) -> str:
    checksum_path = asset_dir / f"{filename}.sha256"
    if not checksum_path.exists():
        raise FileNotFoundError(f"Missing checksum file: {checksum_path}")
    digest = checksum_path.read_text(encoding="utf-8").strip().split()[0]
    if len(digest) != 64:
        raise ValueError(f"Unexpected sha256 digest in {checksum_path}: {digest}")
    return digest


def release_url(repo: str, version: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/v{version}/{filename}"


def write_homebrew_formula(
    output_dir: Path, asset_dir: Path, repo: str, version: str
) -> None:
    linux_filename = asset_name(version, LINUX_X64_TARGET, "tar.gz")
    macos_arm64_filename = asset_name(version, MACOS_ARM64_TARGET, "tar.gz")

    formula = textwrap.dedent(
        f"""\
        class H5v < Formula
          desc "Terminal HDF5 viewer with matrix/chart/image previews"
          homepage "https://github.com/{repo}"
          version "{version}"
          license "Apache-2.0"

          on_macos do
              url "{release_url(repo, version, macos_arm64_filename)}"
              sha256 "{sha256_for(asset_dir, macos_arm64_filename)}"
          end

          on_linux do
            url "{release_url(repo, version, linux_filename)}"
            sha256 "{sha256_for(asset_dir, linux_filename)}"
          end

          def install
            bin.install "h5v"
          end

          test do
            assert_match "HDF5 terminal viewer", shell_output("#{{bin}}/h5v --help")
          end
        end
        """
    )
    formula_path = output_dir / "homebrew" / "h5v.rb"
    formula_path.parent.mkdir(parents=True, exist_ok=True)
    formula_path.write_text(formula, encoding="utf-8")


def write_winget_manifests(
    output_dir: Path, asset_dir: Path, repo: str, version: str
) -> None:
    package_identifier = "DanielHauge.h5v"
    windows_filename = asset_name(version, WINDOWS_X64_TARGET, "zip")
    windows_url = release_url(repo, version, windows_filename)
    windows_sha = sha256_for(asset_dir, windows_filename)
    relative_binary = f"h5v-{WINDOWS_X64_TARGET}-v{version}/h5v.exe"

    manifest_dir = (
        output_dir / "winget" / "manifests" / "d" / "DanielHauge" / "h5v" / version
    )
    manifest_dir.mkdir(parents=True, exist_ok=True)

    version_manifest = textwrap.dedent(
        f"""\
        PackageIdentifier: {package_identifier}
        PackageVersion: {version}
        DefaultLocale: en-US
        ManifestType: version
        ManifestVersion: 1.6.0
        """
    )
    installer_manifest = textwrap.dedent(
        f"""\
        PackageIdentifier: {package_identifier}
        PackageVersion: {version}
        Commands:
          - h5v
        InstallModes:
          - interactive
          - silent
          - silentWithProgress
        Installers:
          - Architecture: x64
            InstallerType: zip
            NestedInstallerType: portable
            NestedInstallerFiles:
              - RelativeFilePath: {relative_binary}
                PortableCommandAlias: h5v
            InstallerUrl: {windows_url}
            InstallerSha256: {windows_sha}
        ManifestType: installer
        ManifestVersion: 1.6.0
        """
    )
    locale_manifest = textwrap.dedent(
        f"""\
        PackageIdentifier: {package_identifier}
        PackageVersion: {version}
        PackageLocale: en-US
        Publisher: Daniel Hauge
        PublisherUrl: https://github.com/DanielHauge
        PublisherSupportUrl: https://github.com/{repo}/issues
        PackageName: h5v
        PackageUrl: https://github.com/{repo}
        ShortDescription: Terminal HDF5 viewer with matrix/chart/image previews, attributes, and scripting.
        Moniker: h5v
        License: Apache-2.0
        LicenseUrl: https://github.com/{repo}/blob/main/LICENSE
        ReleaseNotesUrl: https://github.com/{repo}/releases/tag/v{version}
        Tags:
          - hdf5
          - terminal
          - viewer
          - rust
        ManifestType: defaultLocale
        ManifestVersion: 1.6.0
        """
    )

    (manifest_dir / "DanielHauge.h5v.yaml").write_text(
        version_manifest, encoding="utf-8"
    )
    (manifest_dir / "DanielHauge.h5v.installer.yaml").write_text(
        installer_manifest,
        encoding="utf-8",
    )
    (manifest_dir / "DanielHauge.h5v.locale.en-US.yaml").write_text(
        locale_manifest,
        encoding="utf-8",
    )


def write_scoop_manifest(
    output_dir: Path, asset_dir: Path, repo: str, version: str
) -> None:
    windows_filename = asset_name(version, WINDOWS_X64_TARGET, "zip")
    manifest = {
        "version": version,
        "description": "Terminal HDF5 viewer with matrix/chart/image previews, attributes, and scripting.",
        "homepage": f"https://github.com/{repo}",
        "license": "Apache-2.0",
        "architecture": {
            "64bit": {
                "url": release_url(repo, version, windows_filename),
                "hash": sha256_for(asset_dir, windows_filename),
            }
        },
        "extract_dir": f"h5v-{WINDOWS_X64_TARGET}-v{version}",
        "bin": "h5v.exe",
        "checkver": "github",
        "autoupdate": {
            "architecture": {
                "64bit": {
                    "url": f"https://github.com/{repo}/releases/download/v$version/h5v-{WINDOWS_X64_TARGET}-v$version.zip"
                }
            }
        },
    }
    manifest_path = output_dir / "scoop" / "h5v.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(f"{json.dumps(manifest, indent=2)}\n", encoding="utf-8")


def write_aur_pkgbuild(
    output_dir: Path, asset_dir: Path, repo: str, version: str
) -> None:
    linux_filename = asset_name(version, LINUX_X64_TARGET, "tar.gz")
    pkgbuild = textwrap.dedent(
        f"""\
        pkgname=h5v-bin
        pkgver={version}
        pkgrel=1
        pkgdesc="Terminal HDF5 viewer with matrix/chart/image previews"
        arch=('x86_64')
        url="https://github.com/{repo}"
        license=('Apache-2.0')
        depends=('glibc' 'gcc-libs')
        optdepends=('wl-clipboard: Wayland clipboard integration' 'xclip: X11 clipboard helper')
        source=("h5v-${{pkgver}}.tar.gz::{release_url(repo, version, linux_filename)}")
        sha256sums=('{sha256_for(asset_dir, linux_filename)}')

        package() {{
          install -Dm755 \
            "${{srcdir}}/h5v-{LINUX_X64_TARGET}-v${{pkgver}}/h5v" \
            "${{pkgdir}}/usr/bin/h5v"
        }}
        """
    )
    pkgbuild_path = output_dir / "aur" / "PKGBUILD"
    pkgbuild_path.parent.mkdir(parents=True, exist_ok=True)
    pkgbuild_path.write_text(pkgbuild, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate package manager metadata from release artifacts."
    )
    parser.add_argument("--version", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--asset-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()

    asset_dir = args.asset_dir.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    write_homebrew_formula(output_dir, asset_dir, args.repo, args.version)
    write_winget_manifests(output_dir, asset_dir, args.repo, args.version)
    write_scoop_manifest(output_dir, asset_dir, args.repo, args.version)
    write_aur_pkgbuild(output_dir, asset_dir, args.repo, args.version)


if __name__ == "__main__":
    main()
