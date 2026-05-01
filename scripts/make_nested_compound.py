#!/bin/python

import os

import h5py
import numpy as np


file_name = "nested_compound.h5"
if os.path.exists(file_name):
    os.remove(file_name)

# Create some example data

# Write data and add table-specific attributes
with h5py.File(file_name, "w") as f:
    # make a dataset with a very ensted compound type
    # dt = np.dtype([("a", "i4"), ("b", [("c", "f4"), ("d", "S10")])])
    # even more nested
    dt = np.dtype([("a", "i4"), ("b", [("c", "f4"), ("d", [("e", "S10")])])])
    dset = f.create_dataset(
        "my_nested_compound", data=np.array([(1, (2.5, b"hello"))], dtype=dt)
    )
