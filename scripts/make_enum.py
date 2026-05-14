import h5py
import numpy as np

with h5py.File("enums.h5", "w") as f:
    # enum mapping: name -> value
    enum_mapping = {
        "RED": 0,
        "GREEN": 1,
        "BLUE": 2,
    }

    # create enum dtype
    enum_dtype = h5py.special_dtype(enum=(np.int32, enum_mapping))

    # your data (must match the enum values)
    data = np.array([0, 1, 2, 0, 1, 2], dtype=np.int32)

    # create dataset
    dset = f.create_dataset("my_enum", data=data, dtype=enum_dtype)

    dset.attrs["scalar"] = np.array(1, dtype=enum_dtype)  # GREEN
    fixed_array = np.array([0, 2, 1], dtype=enum_dtype)
    dset.attrs["fixed_array"] = fixed_array
    vlen_enum_dtype = h5py.vlen_dtype(enum_dtype)

    vlen_data = np.array(
        [
            np.array([0, 1], dtype=enum_dtype),
            np.array([2], dtype=enum_dtype),
            np.array([1, 1, 0], dtype=enum_dtype),
        ],
        dtype=object,
    )

    dset.attrs.create("vlen_array", vlen_data, dtype=vlen_enum_dtype)

    scalar_dset = f.create_dataset(
        "scalar_enum", data=np.array(1, dtype=enum_dtype), dtype=enum_dtype
    )
