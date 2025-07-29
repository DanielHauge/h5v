#!/bin/python

import os

import h5py
import numpy as np


file_name = "test_table.h5"
if os.path.exists(file_name):
    os.remove(file_name)

# Define a structured NumPy dtype for the table
dtype = np.dtype(
    [
        ("id", "i4"),  # 32-bit integer
        ("value", "f8"),  # 64-bit float
        ("label", "S10"),  # Fixed-length ASCII string
    ]
)

# Create some example data
data = np.array([(1, 3.14, b"foo"), (2, 2.71, b"bar"), (3, 1.61, b"baz")], dtype=dtype)

# Write data and add table-specific attributes
with h5py.File(file_name, "w") as f:
    dset = f.create_dataset("my_table", data=data)

    # Required attributes for HDF5 table spec
    dset.attrs["CLASS"] = np.bytes_("TABLE")
    dset.attrs["VERSION"] = np.bytes_("1.0")
    dset.attrs["TITLE"] = np.bytes_("Example Table")

    # Column names as FIELD_X_NAME attributes
    for i, name in enumerate(dtype.names):
        attr_name = f"FIELD_{i}_NAME"
        dset.attrs[attr_name] = np.bytes_(name)

print(f"{file_name} created using HDF5 table spec.")
