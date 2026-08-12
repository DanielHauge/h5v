#!/usr/bin/env python3
"""Create string and opaque-data renderer fixtures. Usage: make_test_strings.py [output.h5]."""

import argparse
from pathlib import Path

import h5py
import numpy as np


def main() -> None:
    parser = argparse.ArgumentParser(description="Create HDF5 string renderer fixtures.")
    parser.add_argument("output", nargs="?", default="test_strings.h5")
    output = Path(parser.parse_args().output)

    utf8_vlen = h5py.string_dtype(encoding="utf-8")
    ascii_vlen = h5py.string_dtype(encoding="ascii")
    fixed_utf8 = h5py.string_dtype(encoding="utf-8", length=32)
    opaque16 = h5py.opaque_dtype(np.dtype("V16"))

    with h5py.File(output, "w") as h5:
        h5.create_dataset("utf8_vlen_scalar", data="café 你好 🌍", dtype=utf8_vlen)
        h5.create_dataset(
            "utf8_vlen_array",
            data=np.asarray(["hello", "café", "你好", "emoji 🌍"], dtype=object),
            dtype=utf8_vlen,
        )
        h5.create_dataset("ascii_vlen_scalar", data="plain ASCII", dtype=ascii_vlen)
        h5.create_dataset(
            "ascii_vlen_array",
            data=np.asarray(["alpha", "bravo", "charlie"], dtype=object),
            dtype=ascii_vlen,
        )
        h5.create_dataset("fixed_ascii", data=np.asarray([b"alpha", b"bravo"], dtype="S12"))
        fixed_utf8_dataset = h5.create_dataset("fixed_utf8", shape=(2,), dtype=fixed_utf8)
        fixed_utf8_dataset[...] = np.asarray(["café", "你好"], dtype=object)

        text = h5.create_group("text")
        text.create_dataset("multiline", data="first line\nsecond line\nthird line", dtype=utf8_vlen)
        text.create_dataset(
            "code",
            data="def greet(name):\n    return f'Hello, {name}!'\n",
            dtype=utf8_vlen,
        )
        text.create_dataset(
            "json", data='{"name":"h5v","items":[1,2,3],"enabled":true}', dtype=utf8_vlen
        )

        h5.create_dataset(
            "paged_vlen_strings",
            data=np.asarray([f"page item {i:04d}: café 你好" for i in range(256)], dtype=object),
            dtype=utf8_vlen,
        )

        attributes = h5.create_dataset("string_attributes", data=np.arange(3))
        attributes.attrs.create("vlen_utf8", "attribute café 你好", dtype=utf8_vlen)
        attributes.attrs.create("vlen_ascii", "attribute ASCII", dtype=ascii_vlen)
        attributes.attrs.create("fixed_ascii", b"fixed attribute", dtype="S20")
        attributes.attrs.create(
            "fixed_utf8", np.asarray("fixed café".encode(), dtype="S32"), dtype=fixed_utf8
        )

        h5.create_dataset(
            "opaque_scalar", data=np.asarray(np.void(b"OPAQUE-SCALAR-01"), dtype=opaque16)
        )
        h5.create_dataset(
            "paged_opaque_bytes",
            data=np.asarray(
                [np.void(f"XXD-{i:06d}-BYTES".encode()) for i in range(256)], dtype=opaque16
            ),
            dtype=opaque16,
        )


if __name__ == "__main__":
    main()
