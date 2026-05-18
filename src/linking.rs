use std::collections::HashSet;
use std::path::PathBuf;

use hdf5_metno::File;
use tempfile::Builder;

use crate::{error::AppError, h5f::copy_attr_to_group, importing::ResolvedHdf5Input};

fn unique_link_name(used: &mut HashSet<String>, desired: &str) -> String {
    if used.insert(desired.to_string()) {
        return desired.to_string();
    }

    let mut idx = 2_usize;
    loop {
        let candidate = format!("{desired} ({idx})");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        idx += 1;
    }
}

pub(crate) fn link_resolved(inputs: &[ResolvedHdf5Input]) -> Result<String, AppError> {
    if inputs.is_empty() {
        return Err(AppError::FileError(
            "No resolved inputs were available for linking".to_string(),
        ));
    }

    let mut opened = Vec::with_capacity(inputs.len());
    for input in inputs {
        if !PathBuf::from(&input.hdf5_path).exists() {
            return Err(AppError::FileError(format!(
                "Resolved HDF5 artifact '{}' for '{}' does not exist",
                input.hdf5_path,
                input.original_path.display()
            )));
        }
        opened.push((input, File::open(&input.hdf5_path)?));
    }

    let tmp_dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    let (_reserved_file, tmp_link_path) = Builder::new()
        .prefix("h5v-link-")
        .suffix(".h5")
        .tempfile_in(&tmp_dir)?
        .keep()
        .map_err(|err| AppError::Io(err.error))?;
    let tmp_link_file_path = tmp_link_path.to_string_lossy().into_owned();
    let new_tmp_link_file = File::create(&tmp_link_file_path)?;
    let mut used_names = HashSet::new();

    for (input, hdf5_file) in opened {
        let group_name = unique_link_name(&mut used_names, &input.link_name);
        let fgroup = new_tmp_link_file.create_group(&group_name)?;
        for ds in hdf5_file.datasets()? {
            fgroup.link_external(
                &input.hdf5_path,
                format!("/{}", ds.name()).as_ref(),
                format!("/{group_name}/{}", ds.name()).as_ref(),
            )?;
        }
        for grp in hdf5_file.groups()? {
            fgroup.link_external(
                &input.hdf5_path,
                format!("/{}", grp.name()).as_ref(),
                format!("/{group_name}/{}", grp.name()).as_ref(),
            )?;
        }
        for attr_name in hdf5_file.attr_names()? {
            let attr = hdf5_file.attr(&attr_name)?;
            copy_attr_to_group(&attr, &fgroup, &attr_name)?;
        }
    }

    Ok(tmp_link_file_path)
}
