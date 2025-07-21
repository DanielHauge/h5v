import h5py
import numpy as np

# make a file

with h5py.File("external_data.h5", "w") as f:
    f.create_dataset("my_data", data=np.arange(10))
    f.create_group("my_group")
    f.create_dataset("my_group/my_subdata", data=np.arange(5, 15))


with h5py.File("main.h5", "w") as f:
    # Create an external link to /my_data in external_data.h5
    f["linked_data"] = h5py.ExternalLink("external_data.h5", "/my_data")
    f["linked_group"] = h5py.ExternalLink("external_data.h5", "/my_group")

# try read my_subdata through
