use std::{path::PathBuf, process::exit};

use hdf5_metno::File;
use uuid::Uuid;

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
            // Err(e) => return Err(e),
            Err(_e) => {}
        };
    }
    if hdf5_files.is_empty() {
        eprintln!("None of the files given are valid hdf5 files");
        for path in paths_bufs {
            eprintln!("- {}", path.to_str().unwrap_or_default())
        }
        exit(-1);
    }

    let buf: [u8; 16] = *b"abcdefghijklmnop";
    let uuid = Uuid::new_v8(buf);
    // let uuid = Uuid
    let tmp_dir = dirs::cache_dir()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("/tmp")
        .to_string();
    let tmp_link_file_path = format!("{tmp_dir}/{uuid}.h5");
    let new_tmp_link_file = File::create(&tmp_link_file_path)?;
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
        for _attr_name in hdf5_file.attr_names()? {
            //TODO: Gotta implement attr copying/linking
        }
    }

    Ok(tmp_link_file_path)
}
