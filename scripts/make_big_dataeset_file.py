#!/bin/python

import os

import h5py
import numpy as np

file_name = "big_ds.h5"
if os.path.exists(file_name):
    os.remove(file_name)


with h5py.File(file_name, "w") as f:
    # Make a very big sinus dataset with over 5.000000 elements
    # where a single periiod is about 100000
    x = np.linspace(0, 12500 * 2 * np.pi, 500000000)
    y = np.sin(x)
    f.create_dataset("big_sin_dataset", data=y, dtype=np.float32)
