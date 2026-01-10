import h5py
import numpy as np

# make a file, remove file
# if it exists, and create a dataset with hard and soft links
try:
    import os

    os.remove("many_long_attributes.h5")
except OSError:
    pass

with h5py.File("many_long_attributes.h5", "w") as f:
    f.create_dataset("my_data", data=np.arange(10))
    attr_val = "This is a very long string, much longer that can be rendered under normal situations. But this serves as a way to render it nonetheless.\nAlso maybe we should display it with newline support?"
    f["my_data"].attrs["attr_1"] = attr_val

    f["my_data"].attrs["attr_2"] = attr_val
    f["my_data"].attrs["attr_3"] = attr_val
    f["my_data"].attrs["attr_4"] = attr_val
    f["my_data"].attrs["attr_5"] = attr_val
    f["my_data"].attrs["attr_6"] = attr_val
    f["my_data"].attrs["attr_7"] = attr_val
    f["my_data"].attrs["attr_8"] = attr_val
    f["my_data"].attrs["attr_9"] = attr_val
    f["my_data"].attrs["attr_10"] = attr_val
    f["my_data"].attrs["attr_11"] = attr_val
    f["my_data"].attrs["attr_12"] = attr_val
    f["my_data"].attrs["attr_13"] = attr_val
    f["my_data"].attrs["attr_14"] = attr_val
    f["my_data"].attrs["attr_15"] = attr_val
    f["my_data"].attrs["attr_16"] = attr_val
    f["my_data"].attrs["attr_17"] = attr_val
