#!/bin/python

import os

import h5py
import numpy as np

file_name = "var_arr.h5"
if os.path.exists(file_name):
    os.remove(file_name)


def open_jpg_data(f):
    with open(f, "rb") as f_jpg:
        jpg_data = f_jpg.read()
        jpg_data = np.frombuffer(jpg_data, dtype=np.uint8)
        return jpg_data


with h5py.File(file_name, "w") as f:
    # make an array of random number of random bytes
    initial_size = 3
    dset = f.create_dataset(
        "var_length_arrays",
        (initial_size,),
        dtype=h5py.special_dtype(vlen=np.uint8),
        maxshape=(None,),
    )

    dset.attrs["CLASS"] = "IMAGE"
    dset.attrs["VERSION"] = "1.2"
    dset.attrs["IMAGE_SUBCLASS"] = "IMAGE_JPEG"
    dset[0] = open_jpg_data("./testimages/217-600x600.jpg")
    dset[1] = open_jpg_data("./testimages/372-600x600.jpg")
    dset[2] = open_jpg_data("./testimages/553-600x600.jpg")
    dset.resize((5,))
    dset[3] = open_jpg_data("./testimages/630-600x600.jpg")
    dset[4] = open_jpg_data("./testimages/740-600x600.jpg")
    # resize to 10000 images to stress test
    dset.resize((10000,))
    for i in range(5, 10000):
        dset[i] = open_jpg_data("./testimages/217-600x600.jpg")
