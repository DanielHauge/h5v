#!/bin/python

import h5py
import numpy as np


with h5py.File("test_scientific_notation.h5", "w") as f:
    f["small"] = np.array([1e-6, 2e-5, 3e-4, 4e-3])
    f["large"] = np.array([1e5, 2e6, 3e7, 4e8])
    f["mixed"] = np.array([1e-6, 1e-3, 1.0, 1e3, 1e6])
