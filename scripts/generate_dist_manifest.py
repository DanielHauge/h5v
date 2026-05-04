#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path


ARCHIVE_RE = re.compile(
    r"^h5v-(?P<target>.+)-v(?P<version>.+)\.(?P<ext>tar\.gz|zip)$"
)


def cargo_version_line() -> str:
    try:
        completed = subprocess.run(
            ["cargo", "-V"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return "cargo unknown"
    return completed.stdout.strip()


def executable_asset(target: str) -> dict[str, str]:
    binary_name = "h5v.exe" if target.endswith("windows-msvc") else "h5v"
    return {
        "name": "h5v",
        "path": binary_name,
        "kind": "executable",
    }


def artifact_entry(target: str, filename: str, checksum_name: str | None) -> dict[str, object]:
    entry: dict[str, object] = {
        "name": filename,
        "kind": "executable-zip",
        "target_triples": [target],
        "assets": [
            {"name": "LICENSE", "path": "LICENSE", "kind": "license"},
            {"name": "README.md", "path": "README.md", "kind": "readme"},
            executable_asset(target),
        ],
    }
    if checksum_name is not None:
        entry["checksum"] = checksum_name
    return entry


def checksum_entry(target: str, filename: str) -> dict[str, object]:
    return {
        "name": filename,
        "kind": "checksum",
        "target_triples": [target],
    }


def build_manifest(version: str, repo: str, asset_dir: Path) -> dict[str, object]:
    archives: list[tuple[str, str, str | None]] = []
    artifacts: dict[str, object] = {}

    for path in sorted(asset_dir.iterdir()):
        if not path.is_file():
            continue
        match = ARCHIVE_RE.match(path.name)
        if not match or match.group("version") != version:
            continue

        target = match.group("target")
        checksum_name = f"{path.name}.sha256"
        checksum_path = asset_dir / checksum_name
        checksum_ref = checksum_name if checksum_path.exists() else None

        archives.append((target, path.name, checksum_ref))
        artifacts[path.name] = artifact_entry(target, path.name, checksum_ref)
        if checksum_ref is not None:
            artifacts[checksum_name] = checksum_entry(target, checksum_name)

    if not archives:
        raise SystemExit(f"no release archives matching version {version!r} found in {asset_dir}")

    release_artifacts = []
    for _, archive_name, checksum_name in archives:
        release_artifacts.append(archive_name)
        if checksum_name is not None:
            release_artifacts.append(checksum_name)

    prerelease = "-" in version
    tag = f"v{version}"
    owner, repo_name = repo.split("/", 1)

    return {
        "dist_version": "0.31.0",
        "announcement_tag": tag,
        "announcement_tag_is_implicit": True,
        "announcement_is_prerelease": prerelease,
        "announcement_title": tag,
        "announcement_github_body": "",
        "system_info": {
            "id": "github-actions",
            "cargo_version_line": cargo_version_line(),
            "build_environment": "indeterminate",
        },
        "releases": [
            {
                "app_name": "h5v",
                "app_version": version,
                "artifacts": release_artifacts,
                "hosting": {
                    "github": {
                        "artifact_base_url": "https://github.com",
                        "artifact_download_path": f"/{repo}/releases/download/{tag}/",
                        "owner": owner,
                        "repo": repo_name,
                    }
                },
            }
        ],
        "artifacts": artifacts,
        "publish_prereleases": False,
        "ci": None,
        "linkage": [],
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate a cargo-dist-compatible release manifest for Oranda."
    )
    parser.add_argument("--version", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--asset-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    manifest = build_manifest(
        version=args.version,
        repo=args.repo,
        asset_dir=args.asset_dir.resolve(),
    )
    args.output.write_text(f"{json.dumps(manifest, indent=2)}\n", encoding="utf-8")


if __name__ == "__main__":
    main()
