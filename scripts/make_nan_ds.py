#!/bin/python

import os

import h5py
import numpy as np


file_name = "test_nan_ds.h5"
if os.path.exists(file_name):
    os.remove(file_name)

# Create some example data

# Write data and add table-specific attributes
with h5py.File(file_name, "w") as f:
    # Make a dataset with nans
    dset = f.create_dataset("my_nans", data=np.array([np.nan, np.nan], dtype="f4"))
    # make one with just a single nan value
    dset2 = f.create_dataset("my_single_nan", data=np.array(np.nan, dtype="f4"))
