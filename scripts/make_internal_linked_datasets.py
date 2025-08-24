import h5py
import numpy as np

# make a file, remove file
# if it exists, and create a dataset with hard and soft links
try:
    import os

    os.remove("internal_links_datasets.h5")
except OSError:
    pass

with h5py.File("internal_links_datasets.h5", "w") as f:
    ds = f.create_dataset("my_data", data=np.arange(10))
    # create soft link
    f["soft_linked_data"] = h5py.SoftLink(ds.name)
