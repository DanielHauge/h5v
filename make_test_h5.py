#!/bin/python

import os

import h5py
import numpy as np

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
    # also some arrays
    f["attributes_ds"].attrs["array"] = np.array([1, 2, 3, 4, 5])
    f["attributes_ds"].attrs["array2"] = np.array([1.0, 2.0, 3.0, 4.0, 5.0])
    f["attributes_ds"].attrs["array3"] = np.array(
        [True, False, True, False, True])
    f["attributes_ds"].attrs["array4"] = np.array([b"hello", b"world"])
    f["attributes_ds"].attrs["array5"] = np.array(
        [b"hello", b"world"], dtype="S")
    f["attributes_ds"].attrs["array6"] = np.array(
        [b"hello", b"world"], dtype="|S5")
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
