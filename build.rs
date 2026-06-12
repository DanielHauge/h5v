use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));

    if let Some(git_dir) = resolve_git_dir(&manifest_dir) {
        emit_git_rerun_hints(&git_dir);
    }

    let fallback = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION missing");
    let full_version = git_describe(
        &manifest_dir,
        &[
            "describe",
            "--always",
            "--dirty=-modified",
            "--tags",
            "--abbrev=4",
        ],
    )
    .unwrap_or_else(|| fallback.clone());
    let short_version =
        git_describe(&manifest_dir, &["describe", "--tags", "--abbrev=0"]).unwrap_or(fallback);

    println!("cargo:rustc-env=H5V_GIT_VERSION={full_version}");
    println!("cargo:rustc-env=H5V_GIT_VERSION_SHORT={short_version}");
}

fn git_describe(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8(output.stdout).ok()?;
    let version = version.trim();
    (!version.is_empty()).then(|| version.to_string())
}

fn resolve_git_dir(manifest_dir: &Path) -> Option<PathBuf> {
    let git_path = manifest_dir.join(".git");
    if git_path.is_dir() {
        return Some(git_path);
    }

    let gitdir = fs::read_to_string(git_path).ok()?;
    let gitdir = gitdir.trim().strip_prefix("gitdir: ")?;
    let gitdir = Path::new(gitdir);
    Some(if gitdir.is_absolute() {
        gitdir.to_path_buf()
    } else {
        manifest_dir.join(gitdir)
    })
}

fn emit_git_rerun_hints(git_dir: &Path) {
    for path in ["HEAD", "index", "packed-refs"] {
        println!("cargo:rerun-if-changed={}", git_dir.join(path).display());
    }

    let head_path = git_dir.join("HEAD");
    let Ok(head) = fs::read_to_string(head_path) else {
        return;
    };
    let Some(reference) = head.trim().strip_prefix("ref: ") else {
        return;
    };
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join(reference).display()
    );
}
