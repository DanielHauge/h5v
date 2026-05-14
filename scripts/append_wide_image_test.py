#!/bin/python

import argparse
from datetime import UTC, datetime
from pathlib import Path

import h5py
import numpy as np


DEFAULT_FILE = "wide_image_test.h5"


def next_patch_start(dataset, patch_width):
    marker = int(dataset.attrs.get("H5V_TEST_PATCH_START", 0))
    max_start = max(dataset.shape[1] - patch_width, 0)
    return min(marker, max_start)


def apply_visible_patch(dataset, patch_width, intensity):
    if dataset.ndim != 2:
        return None

    width = min(max(patch_width, 1), dataset.shape[1])
    start = next_patch_start(dataset, width)
    end = start + width
    original = dataset[:, start:end]

    boosted = np.clip(original.astype(np.int16) + intensity, 0, 255).astype(np.uint8)
    boosted[::16, :] = 255
    dataset[:, start:end] = boosted

    next_start = 0 if end >= dataset.shape[1] else end
    dataset.attrs["H5V_TEST_PATCH_START"] = next_start
    dataset.attrs["H5V_TEST_PATCH_RANGE"] = f"{start}:{end}"
    return start, end


def append_reload_log(file_handle):
    timestamp = datetime.now(UTC).isoformat()
    message = np.asarray([timestamp], dtype=h5py.string_dtype("utf-8"))

    if "reload_log" in file_handle:
        dataset = file_handle["reload_log"]
        old_size = dataset.shape[0]
        dataset.resize((old_size + 1,))
        dataset[old_size] = message[0]
    else:
        dataset = file_handle.create_dataset(
            "reload_log",
            data=message,
            maxshape=(None,),
            chunks=(32,),
            dtype=h5py.string_dtype("utf-8"),
        )

    file_handle.attrs["H5V_LAST_APPEND_AT"] = timestamp
    file_handle.attrs["H5V_APPEND_COUNT"] = dataset.shape[0]
    return dataset.shape[0]


def main():
    parser = argparse.ArgumentParser(
        description="Append a visible test change to wide_image_test.h5 for reload testing."
    )
    parser.add_argument("file", nargs="?", default=DEFAULT_FILE)
    parser.add_argument("--patch-width", type=int, default=96)
    parser.add_argument("--intensity", type=int, default=80)
    args = parser.parse_args()

    path = Path(args.file)
    if not path.exists():
        raise SystemExit(f"File not found: {path}")

    with h5py.File(path, "a") as file_handle:
        log_count = append_reload_log(file_handle)

        patch_range = None
        if "wide_radargram" in file_handle:
            patch_range = apply_visible_patch(
                file_handle["wide_radargram"],
                patch_width=args.patch_width,
                intensity=args.intensity,
            )

        file_handle.flush()

    print(f"Appended reload_log entry #{log_count} in {path}")
    if patch_range is not None:
        start, end = patch_range
        print(f"Brightened wide_radargram columns {start}..{end}")


if __name__ == "__main__":
    main()
