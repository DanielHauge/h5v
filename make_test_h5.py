#!/bin/python

import os

import h5py
import numpy as np
from PIL import Image

file_name = "test.h5"
if os.path.exists(file_name):
    os.remove(file_name)

with h5py.File(file_name, "w") as f:
    # Create a dataset with random data
    data = np.random.random((100, 100))
    f.create_dataset("attributes_ds", data=data)

    # Create a group and add a dataset to it
    group = f.create_group("group_1")
    group.create_dataset("dataset_2", data=data)

    # Add attributes to the dataset
    f["attributes_ds"].attrs["description"] = "This is a random dataset"
    f["attributes_ds"].attrs["units"] = "arbitrary units"
    f["attributes_ds"].attrs["author"] = "Your Name"

    # Create string dataset from unicode string, not attribute
    my_unicode_string = "你好，世界! 🌍"
    dt = h5py.string_dtype(encoding="utf-8")
    f.create_dataset("unicode_string", data=my_unicode_string, dtype=dt)

    # also some arrays
    f["attributes_ds"].attrs["array"] = np.array([1, 2, 3, 4, 5])
    f["attributes_ds"].attrs["array2"] = np.array([1.0, 2.0, 3.0, 4.0, 5.0])
    f["attributes_ds"].attrs["array3"] = np.array([True, False, True, False, True])
    f["attributes_ds"].attrs["array4"] = np.array([b"hello", b"world"])
    f["attributes_ds"].attrs["array5"] = np.array([b"hello", b"world"], dtype="S")
    f["attributes_ds"].attrs["array6"] = np.array([b"hello", b"world"], dtype="|S5")
    f["attributes_ds"].attrs["float"] = 3.14
    f["attributes_ds"].attrs["float_array"] = np.array([3.14, 2.71, 1.41])
    f["attributes_ds"].attrs["int"] = 42
    f["attributes_ds"].attrs["bool"] = True

    # Make a bigger nested groups and random good things
    group_2 = f.create_group("group_1/group_2")
    group_2.create_dataset("dataset_3", data=data)
    group_2.create_dataset("dataset_4", data=data)
    group_2.create_dataset("dataset_5", data=data)
    group_3 = f.create_group("group_1/group_3")
    group_3.create_dataset("dataset_6", data=data)
    group_3.create_dataset("dataset_7", data=data)
    group_3.create_dataset("dataset_8", data=data)
    group_4 = group_3.create_group("group_4")
    group_4.create_dataset("dataset_9", data=data)
    group_4.create_dataset("dataset_10", data=data)
    group_4.create_dataset("dataset_11", data=data)

    # Big dataset, 1 gb
    num_points = 268_435_456
    # 5 full sine waves over the entire data
    x = np.linspace(0, 10 * np.pi, num_points)
    y = np.sin(x).astype(np.float32)
    f.create_dataset("big_dataset", data=y)

    # # BIG dataset, 10 gb
    # num_points = 2**30
    # # 5 full sine waves over the entire data
    # x = np.linspace(0, 10 * np.pi, num_points)
    # y = np.sin(x).astype(np.float32)
    # f.create_dataset("big_dataset_2", data=y)

    # make grp for all datasets of all types
    group_all = f.create_group("all_datasets")
    # u8
    x = np.random.randint(0, 255, size=(1, 100), dtype=np.uint8)
    group_all.create_dataset("uint8_dataset", data=x)
    # u16
    x = np.random.randint(0, 65535, size=(1, 100), dtype=np.uint16)
    group_all.create_dataset("uint16_dataset", data=x)
    # u32
    x = np.random.randint(0, 4294967295, size=(1, 100), dtype=np.uint32)
    group_all.create_dataset("uint32_dataset", data=x)
    # u64
    x = np.random.randint(0, 18446744073709551615, size=(1, 100), dtype=np.uint64)
    group_all.create_dataset("uint64_dataset", data=x)
    # i8
    x = np.random.randint(-128, 127, size=(1, 100), dtype=np.int8)
    group_all.create_dataset("int8_dataset", data=x)
    # i16
    x = np.random.randint(-32768, 32767, size=(1, 100), dtype=np.int16)
    group_all.create_dataset("int16_dataset", data=x)
    # i32
    x = np.random.randint(-2147483648, 2147483647, size=(1, 100), dtype=np.int32)
    group_all.create_dataset("int32_dataset", data=x)
    # i64
    x = np.random.randint(
        -9223372036854775808, 9223372036854775807, size=(1, 100), dtype=np.int64
    )
    group_all.create_dataset("int64_dataset", data=x)

    # f32
    x = np.random.random((1, 100)).astype(np.float32)
    group_all.create_dataset("float32_dataset", data=x)
    # f64
    x = np.random.random((1, 100)).astype(np.float64)
    group_all.create_dataset("float64_dataset", data=x)

    # u32
    x = np.random.randint(0, 255, size=(100, 1), dtype=np.uint8)
    group_all.create_dataset("chunked_first_by_one", data=x)

    # Create some chunking dataset like 10x4096x150
    x = np.random.random((10, 4096, 150))
    f.create_dataset("chunked_dataset", data=x, chunks=(1, 1024, 150))

    # sinusoidal dataset
    x = np.linspace(0, 2 * np.pi, 100)
    y = np.sin(x)
    f.create_dataset("sinusoidal_dataset", data=y)

    # Some other pretty pattern dataset
    x = np.linspace(0, 2 * np.pi, 100)
    y = np.cos(x)
    f.create_dataset("cosine_dataset", data=y)

    # Some other pretty pattern dataset
    x = np.linspace(0, 2 * np.pi, 100)
    y = np.tan(x)
    f.create_dataset("tangent_dataset", data=y)

    # Some other pretty pattern dataset NOT sinusoidal
    x = np.linspace(0, 2 * np.pi, 100)
    y = np.sinh(x)
    f.create_dataset("sinh_dataset", data=y)

    # some cool pattern
    x = np.linspace(0, 2 * np.pi, 100)
    y = np.cosh(x)
    f.create_dataset("cosh_dataset", data=y)

    # some cool pattern
    x = np.linspace(0, 10 * np.pi, 1000)
    y = np.sin(x) + np.random.normal(0, 0.3, size=x.shape)
    f.create_dataset("sinusoidal_with_noise", data=y)

    a, b, delta = 5, 4, np.pi / 2
    t = np.linspace(0, 2 * np.pi, 1000)
    x = np.sin(a * t + delta)
    y = np.sin(b * t)
    f.create_dataset("parametric_curve", data=np.array([x, y]).T)

    theta = np.linspace(0, 4 * np.pi, 1000)
    r = theta + np.random.normal(0, 0.2, size=theta.shape)
    x = r * np.cos(theta)
    y = r * np.sin(theta)
    f.create_dataset("polar_curve", data=np.array([x, y]).T)

    x = np.linspace(0, 20 * np.pi, 1000)
    y = np.sin(x) + np.sin(1.1 * x)
    f.create_dataset("beat_pattern", data=y)

    steps = np.random.choice([-1, 1], size=1000)
    path = np.cumsum(steps)
    f.create_dataset("random_walk", data=path)

    # simple string dataset
    f.create_dataset("string_dataset", data="Hello string")

    # also an ascii string dataset
    f.create_dataset("ascii_string_dataset", data=b"Hello ascii")
    # also a unicode string dataset
    f.create_dataset("unicode_string_dataset", data="Hello utf8".encode("utf-8"))

    f.create_dataset("bytes_dataset", data=b"Hello bytes")

    # open jpeg file and save it in the hdf5 file
    with open("test.jpg", "rb") as f_jpg:
        jpg_data = f_jpg.read()
        # as u8 array
        jpg_data = np.frombuffer(jpg_data, dtype=np.uint8)
        f.create_dataset("jpg_dataset", data=jpg_data)
        # set attributes: CLASS: IMAGE, VERSION: 1.2
        # set attribute IMAGE: JPEG
        f["jpg_dataset"].attrs["CLASS"] = "IMAGE"
        f["jpg_dataset"].attrs["VERSION"] = "1.2"
        f["jpg_dataset"].attrs["IMAGE_SUBCLASS"] = "IMAGE_JPEG"
        # make official jpeg into true color color HDF5 stuff

    img = Image.open("test.jpg").convert("RGB")
    img_array = np.array(img)
    # save the image as a dataset
    image_ds = f.create_dataset("image_rgb", data=img_array)
    image_ds.attrs["CLASS"] = "IMAGE"
    image_ds.attrs["IMAGE_SUBCLASS"] = "IMAGE_TRUECOLOR"
    image_ds.attrs["IMAGE_VERSION"] = "1.2"
    # inteflace
    image_ds.attrs["INTERLACE_MODE"] = "INTERLACE_PIXEL"
    # aspect ratio
    image_ds.attrs["IMAGE_ASPECTRATIO"] = 10

    # grayscale image
    img_gray = Image.open("test.jpg").convert("L")
    img_gray_array = np.array(img_gray)
    # save the image as a dataset
    image_gray_ds = f.create_dataset("image_gray", data=img_gray_array)
    image_gray_ds.attrs["CLASS"] = "IMAGE"
    image_gray_ds.attrs["IMAGE_SUBCLASS"] = "IMAGE_GRAYSCALE"
    image_gray_ds.attrs["IMAGE_VERSION"] = "1.2"
    # image white is zero usigned interger importantt
    image_gray_ds.attrs.create("IMAGE_WHITE_IS_ZERO", 0, dtype=np.uint8)

    # bitmap
    img_bitmap = Image.open("test.jpg").convert("1")
    img_bitmap_array = np.array(img_bitmap)
    # save the image as a dataset
    image_bitmap_ds = f.create_dataset("image_bitmap", data=img_bitmap_array)
    image_bitmap_ds.attrs["CLASS"] = "IMAGE"
    image_bitmap_ds.attrs["IMAGE_SUBCLASS"] = "IMAGE_BITMAP"
    image_bitmap_ds.attrs["IMAGE_VERSION"] = "1.2"
    image_bitmap_ds.attrs.create("IMAGE_WHITE_IS_ZERO", 0, dtype=np.uint8)

    # indexed pallet
    img_indexed = Image.open("test.jpg").convert("P")
    img_indexed_array = np.array(img_indexed)
    # max 256 colors, so 256*3 = 768 entries
    palette = img_indexed.getpalette()[:768]

    palette_np = np.array(palette, dtype=np.uint8).reshape(-1, 3)  # shape (N, 3)
    # save the palette as a dataset
    palette_ds = f.create_dataset("palette", data=palette_np, dtype=np.uint8)
    # save the image as a dataset
    image_indexed_ds = f.create_dataset("image_indexed", data=img_indexed_array)
    image_indexed_ds.attrs["CLASS"] = "IMAGE"
    image_indexed_ds.attrs["IMAGE_SUBCLASS"] = "IMAGE_INDEXED"
    image_indexed_ds.attrs["IMAGE_VERSION"] = "1.2"
    image_indexed_ds.attrs["PALETTE"] = palette_ds.ref
    image_indexed_ds.attrs["INTERLACE_MODE"] = "INTERLACE_PIXEL"

    # do png
    with open("test.png", "rb") as f_png:
        png_data = f_png.read()
        # as u8 array
        png_data = np.frombuffer(png_data, dtype=np.uint8)
        f.create_dataset("png_dataset", data=png_data)
        # set attributes: CLASS: IMAGE, VERSION: 1.2
        # set attribute IMAGE: PNG
        f["png_dataset"].attrs["CLASS"] = "IMAGE"
        f["png_dataset"].attrs["VERSION"] = "1.2"
        f["png_dataset"].attrs["IMAGE_SUBCLASS"] = "IMAGE_PNG"
