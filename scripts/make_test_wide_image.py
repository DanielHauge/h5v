#!/bin/python

import os

import h5py
import numpy as np


FILE_NAME = "wide_image_test.h5"


def image_attrs(dataset, subclass):
    dataset.attrs["CLASS"] = "IMAGE"
    dataset.attrs["IMAGE_SUBCLASS"] = subclass
    dataset.attrs["IMAGE_VERSION"] = "1.2"
    if subclass == "IMAGE_GRAYSCALE":
        dataset.attrs.create("IMAGE_WHITE_IS_ZERO", 0, dtype=np.uint8)


def make_radargram(height, width):
    rows = np.linspace(0.0, 1.0, height, dtype=np.float32)[:, None]
    cols = np.linspace(0.0, 1.0, width, dtype=np.float32)[None, :]

    layered = 0.45 + 0.25 * np.sin(cols * 180.0 + rows * 22.0)
    reflectors = 0.18 * np.sin(cols * 850.0) * np.exp(-rows * 2.5)
    diagonal = 0.14 * np.sin((cols * 42.0 - rows * 11.0) ** 2)
    attenuation = np.exp(-rows * 1.7)

    image = (layered + reflectors + diagonal) * attenuation
    image += 0.08 * np.sin(rows * 70.0)
    image = np.clip(image, 0.0, 1.0)
    return (image * 255.0).astype(np.uint8)


def main():
    if os.path.exists(FILE_NAME):
        os.remove(FILE_NAME)

    wide = make_radargram(512, 60000)
    tall = np.rot90(make_radargram(384, 18000))

    with h5py.File(FILE_NAME, "w") as f:
        wide_ds = f.create_dataset(
            "wide_radargram",
            data=wide,
            dtype=np.uint8,
            chunks=(64, 2048),
        )
        image_attrs(wide_ds, "IMAGE_GRAYSCALE")

        tall_ds = f.create_dataset(
            "tall_radargram",
            data=tall,
            dtype=np.uint8,
            chunks=(2048, 64),
        )
        image_attrs(tall_ds, "IMAGE_GRAYSCALE")


if __name__ == "__main__":
    main()
