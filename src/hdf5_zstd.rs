use crate::error::AppError;

#[cfg(test)]
const HDF5_ZSTD_FILTER_ID: i32 = 32015;

unsafe extern "C" {
    fn h5v_register_hdf5_zstd_filter() -> std::ffi::c_int;
}

pub(crate) fn register() -> Result<(), AppError> {
    if unsafe { h5v_register_hdf5_zstd_filter() } < 0 {
        return Err(AppError::FileError(
            "Failed to register the built-in HDF5 Zstandard filter (ID 32015)".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_filter_is_available_after_registration() {
        let _guard = crate::test_support::hdf5_test_guard();
        register().expect("register HDF5 Zstandard filter");
        assert!(unsafe { hdf5_metno_sys::h5z::H5Zfilter_avail(HDF5_ZSTD_FILTER_ID) } > 0);
    }
}
