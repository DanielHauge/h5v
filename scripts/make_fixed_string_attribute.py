import h5py
import numpy as np

try:
    import os

    os.remove("fixed_str_attr.h5")
except OSError:
    pass

with h5py.File("fixed_str_attr.h5", "w") as f:
    fixed_str = "This is a fixed string attribute"
    fixed_dtype = h5py.string_dtype(encoding="utf-8", length=len(fixed_str))
    ds = f.create_dataset("data", data=[1, 2, 3], dtype=np.int32)
    ds.attrs.create("FIXED_STR_ATTR", fixed_str, dtype=fixed_dtype)
