use std::sync::{LazyLock, Mutex, MutexGuard};

static SERIAL_TEST_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static HDF5_TEST_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[allow(clippy::expect_used)]
pub(crate) fn serial_test_guard() -> MutexGuard<'static, ()> {
    match SERIAL_TEST_GUARD.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[allow(clippy::expect_used)]
pub(crate) fn hdf5_test_guard() -> MutexGuard<'static, ()> {
    match HDF5_TEST_GUARD.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
