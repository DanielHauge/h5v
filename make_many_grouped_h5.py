#!/bin/python

import os

import h5py
import numpy as np

file_name = "test_grp.h5"
if os.path.exists(file_name):
    os.remove(file_name)

with h5py.File(file_name, "w") as f:
    # Create a dataset with random data
    data = np.random.random((100, 100))

    group = f.create_group("group_1")
    # make 20000 datasets in group_1 that just increments the name
    for i in range(1000):
        group.create_dataset(f"dataset_{i}", data=data)
