#!/usr/bin/env python3

import os

import h5py
import numpy as np
from PIL import Image

file_name = "tomany.h5"
if os.path.exists(file_name):
    os.remove(file_name)

with h5py.File(file_name, "w") as f:
    group = f.create_group("many_datasets")
    # Make lots of datasets like 1000
    for i in range(1000):
        data = np.random.random((100, 100))
        group.create_dataset(f"dataset_{i}", data=data)
