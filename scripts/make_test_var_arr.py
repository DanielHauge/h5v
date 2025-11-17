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


def download_file_url(url, filename):
    import requests

    response = requests.get(url)
    with open(filename, "wb") as f:
        f.write(response.content)
    print(f"Downloaded {filename} from {url}")


with h5py.File(file_name, "w") as f:
    # make an array of random number of random bytes
    initial_size = 3
    dset = f.create_dataset(
        "var_length_arrays",
        (initial_size,),
        dtype=h5py.special_dtype(vlen=np.uint8),
        maxshape=(None,),
    )
    # https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.G37tgeQqSNt7v2oPfj9ltQHaE7%3Fpid%3DApi&f=1&ipt=d9c8e98ab758833cc80880ecb836f6f7c50b4fe16f44001a19de82f76036c4ef&ipo=images
    # https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.7cRYFyLoDEDh4sRtM73vvwHaDg%3Fpid%3DApi&f=1&ipt=1fa76cb884b06fda5cb88ead0989f34665c63d2de8452765c9169b3efbcd075a&ipo=images
    # https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.uHaqRdiMzWSMCR2LzsmhtQHaEZ%3Fpid%3DApi&f=1&ipt=50d793d835eab573830c354ace883883fe2be0fa30871bb02255a411c3ab96ad&ipo=images
    # https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.jpLd-_FFm5nktmj6TtNtHAHaFj%3Fpid%3DApi&f=1&ipt=7c7cd79d629ee428c2ac8837527e9aa09e562ef6cc1b39024268456c5b698c02&ipo=images
    # https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.QVBY30VqTi-tlYt_BaoGqAHaEo%3Fpid%3DApi&f=1&ipt=4ec2b8eb9eb6571d4dabcd9210b5fa4c7738cad990b2012984894737a2e2cd88&ipo=images
    # Download them and add them
    download_file_url(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.G37tgeQqSNt7v2oPfj9ltQHaE7%3Fpid%3DApi&f=1&ipt=d9c8e98ab758833cc80880ecb836f6f7c50b4fe16f44001a19de82f76036c4ef&ipo=images",
        "./testimages/img1.jpg",
    )
    download_file_url(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.7cRYFyLoDEDh4sRtM73vvwHaDg%3Fpid%3DApi&f=1&ipt=1fa76cb884b06fda5cb88ead0989f34665c63d2de8452765c9169b3efbcd075a&ipo=images",
        "./testimages/img2.jpg",
    )
    download_file_url(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.uHaqRdiMzWSMCR2LzsmhtQHaEZ%3Fpid%3DApi&f=1&ipt=50d793d835eab573830c354ace883883fe2be0fa30871bb02255a411c3ab96ad&ipo=images",
        "./testimages/img3.jpg",
    )
    download_file_url(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.jpLd-_FFm5nktmj6TtNtHAHaFj%3Fpid%3DApi&f=1&ipt=7c7cd79d629ee428c2ac8837527e9aa09e562ef6cc1b39024268456c5b698c02&ipo=images",
        "./testimages/img4.jpg",
    )
    download_file_url(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ftse1.mm.bing.net%2Fth%2Fid%2FOIP.QVBY30VqTi-tlYt_BaoGqAHaEo%3Fpid%3DApi&f=1&ipt=4ec2b8eb9eb6571d4dabcd9210b5fa4c7738cad990b2012984894737a2e2cd88&ipo=images",
        "./testimages/img5.jpg",
    )

    dset.attrs["CLASS"] = "IMAGE"
    dset.attrs["VERSION"] = "1.2"
    dset.attrs["IMAGE_SUBCLASS"] = "IMAGE_JPEG"
    dset[0] = open_jpg_data("./testimages/img1.jpg")
    dset[1] = open_jpg_data("./testimages/img2.jpg")
    dset[2] = open_jpg_data("./testimages/img3.jpg")
    dset.resize((5,))
    dset[3] = open_jpg_data("./testimages/img4.jpg")
    dset[4] = open_jpg_data("./testimages/img5.jpg")
    # resize to 10000 images to stress test
    dset.resize((50,))
    for i in range(5, 50):
        dset[i] = open_jpg_data("./testimages/img1.jpg")
