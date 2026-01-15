use std::path::PathBuf;

use hdf5_metno::File;

use crate::error::AppError;

pub fn link(paths: &[String]) -> Result<String, AppError> {
    let paths_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    for x in &paths_bufs {
        if !x.exists() {
            return Err(AppError::FileError(format!("{x:?} doesn't exist")));
        }
    }
    let hdf5_file_results = paths_bufs
        .iter()
        .map(File::open)
        .map(|x| x.map_err(AppError::from));
    let mut hdf5_files = vec![];
    for hdf5_file in hdf5_file_results {
        match hdf5_file {
            Ok(f) => hdf5_files.push(f),
            Err(e) => return Err(e),
        };
    }

    let new_tmp_link_file_path = "test.link.h5";
    let new_tmp_link_file = File::create(new_tmp_link_file_path)?;
    for hdf5_file in hdf5_files {
        let fname = hdf5_file.filename();
        let fgroup = new_tmp_link_file.create_group(fname.as_ref())?;
        for ds in hdf5_file.datasets()? {
            fgroup.link_external(
                &fname,
                format!("/{}", ds.name()).as_ref(),
                format!("/{}/{}", fname, ds.name()).as_ref(),
            )?;
        }
        for grp in hdf5_file.groups()? {
            fgroup.link_external(
                &fname,
                format!("/{}", grp.name()).as_ref(),
                format!("/{}/{}", fname, grp.name()).as_ref(),
            )?;
        }
        for attr_name in hdf5_file.attr_names()? {
            //TODO: Gotta implement attr copying/linking
        }
    }

    Ok(String::from(new_tmp_link_file_path))
}
