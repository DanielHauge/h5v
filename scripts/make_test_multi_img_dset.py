#!/bin/python

import os
from PIL import Image

import h5py
import numpy as np

file_name = "multi_img.h5"
if os.path.exists(file_name):
    os.remove(file_name)


def open_jpg_data(f):
    with Image.open(f) as img:
        img = img.convert("RGB")  # Ensure the image is in RGB format
        return np.array(img, dtype=np.uint8)


with h5py.File(file_name, "w") as f:
    img_1 = open_jpg_data("./testimages/217-600x600.jpg")
    img_2 = open_jpg_data("./testimages/372-600x600.jpg")
    img_3 = open_jpg_data("./testimages/553-600x600.jpg")
    img_4 = open_jpg_data("./testimages/630-600x600.jpg")
    img_5 = open_jpg_data("./testimages/740-600x600.jpg")

    # Make a 5 x 600 x 600 x 3 dataset
    dset = f.create_dataset(
        "multi_img_dataset",
        (5, 600, 600, 3),
        dtype=np.uint8,
        chunks=(1, 600, 600, 3),
    )
    dset.attrs["CLASS"] = "IMAGE"
    dset.attrs["VERSION"] = "1.2"
    dset.attrs["IMAGE_SUBCLASS"] = "IMAGE_TRUECOLOR"
    dset.attrs["INTERLACE_MODE"] = "INTERLACE_PIXEL"
    dset[0] = img_1
    dset[1] = img_2
    dset[2] = img_3
    dset[3] = img_4
    dset[4] = img_5

    # Make a single 600 x 600 x 3 dataset
    dset_single = f.create_dataset(
        "single_img_dataset",
        (600, 600, 3),
        dtype=np.uint8,
        chunks=(600, 600, 3),
        data=img_1,  # Using the first image as an example
    )
    dset_single.attrs["CLASS"] = "IMAGE"
    dset_single.attrs["VERSION"] = "1.2"
    dset_single.attrs["IMAGE_SUBCLASS"] = "IMAGE_TRUECOLOR"
    dset_single.attrs["INTERLACE_MODE"] = "INTERLACE_PIXEL"
