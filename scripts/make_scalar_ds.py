#!/bin/python

import os

import h5py
import numpy as np


file_name = "test_scalar_ds.h5"
if os.path.exists(file_name):
    os.remove(file_name)

# Create some example data

# Write data and add table-specific attributes
with h5py.File(file_name, "w") as f:
    # Make a dataset with a single scalar value
    dset = f.create_dataset("my_scalar", data=np.array(42, dtype="i4"))
    # make one with just 2 values
    dset2 = f.create_dataset("my_scalar2", data=np.array([42, 43], dtype="i4"))
