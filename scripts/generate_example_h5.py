#!/usr/bin/env python3

from __future__ import annotations

from io import BytesIO
from pathlib import Path

import h5py
import numpy as np
from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = ROOT / "examples" / "h5v-example.h5"


def utf8_dtype():
    return h5py.string_dtype(encoding="utf-8")


def image_attrs(dataset, subclass: str, interlace: str | None = None) -> None:
    dataset.attrs["CLASS"] = "IMAGE"
    dataset.attrs["IMAGE_SUBCLASS"] = subclass
    dataset.attrs["VERSION"] = "1.2"
    dataset.attrs["IMAGE_VERSION"] = "1.2"
    if interlace is not None:
        dataset.attrs["INTERLACE_MODE"] = interlace
    if subclass in {"IMAGE_GRAYSCALE", "IMAGE_BITMAP"}:
        dataset.attrs.create("IMAGE_WHITE_IS_ZERO", 0, dtype=np.uint8)


def encode_image_bytes(array: np.ndarray, image_format: str) -> np.ndarray:
    image = Image.fromarray(array)
    buffer = BytesIO()
    save_args = {"format": image_format}
    if image_format == "JPEG":
        save_args.update({"quality": 82, "optimize": True})
    image.save(buffer, **save_args)
    return np.frombuffer(buffer.getvalue(), dtype=np.uint8)


def make_rgb_image(height: int, width: int, phase: float) -> np.ndarray:
    rows = np.linspace(0.0, 1.0, height, dtype=np.float32)[:, None]
    cols = np.linspace(0.0, 1.0, width, dtype=np.float32)[None, :]
    red = np.broadcast_to(
        0.55 + 0.35 * np.sin((cols * 7.0 + phase) * np.pi), (height, width)
    )
    green = np.broadcast_to(
        0.45 + 0.45 * np.cos((rows * 5.0 - phase) * np.pi), (height, width)
    )
    blue = 0.35 + 0.55 * np.sin((rows * 3.0 + cols * 2.0 + phase) * np.pi)
    image = np.stack([red, green, blue], axis=-1)
    return np.clip(image * 255.0, 0.0, 255.0).astype(np.uint8)


def make_grayscale_image(height: int, width: int, frequency: float) -> np.ndarray:
    rows = np.linspace(0.0, 1.0, height, dtype=np.float32)[:, None]
    cols = np.linspace(0.0, 1.0, width, dtype=np.float32)[None, :]
    bands = 0.5 + 0.25 * np.sin(cols * frequency * np.pi)
    diagonal = 0.22 * np.sin((cols * 6.0 - rows * 4.0) * np.pi)
    attenuation = np.exp(-rows * 1.6)
    image = np.clip((bands + diagonal) * attenuation + rows * 0.35, 0.0, 1.0)
    return (image * 255.0).astype(np.uint8)


def build_example_file(output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    if output_path.exists():
        output_path.unlink()

    vlen_u8 = h5py.vlen_dtype(np.dtype("uint8"))
    enum_dtype = h5py.special_dtype(
        enum=(np.uint8, {"LOW": 0, "MEDIUM": 1, "HIGH": 2})
    )

    with h5py.File(output_path, "w") as h5:
        h5.attrs["title"] = "h5v bundled example"
        h5.attrs["description"] = (
            "Compact demo file for charts, matrices, images, compounds, links, and references."
        )
        h5.attrs["version"] = "1.0"

        scalars = h5.create_group("scalars")
        scalars.create_dataset("int_scalar", data=np.int32(42))
        scalars.create_dataset("float_scalar", data=np.float64(np.pi))
        scalars.create_dataset("bool_scalar", data=np.bool_(True))
        scalars.create_dataset("unicode_text", data="hello from h5v", dtype=utf8_dtype())

        signals = h5.create_group("signals")
        sample_axis = np.linspace(0.0, 4.0 * np.pi, 128, dtype=np.float32)
        sine = np.sin(sample_axis).astype(np.float32)
        cosine = np.cos(sample_axis).astype(np.float32)
        mixed = (0.7 * np.sin(sample_axis) + 0.2 * np.cos(sample_axis * 2.0)).astype(
            np.float32
        )
        signal_ds = signals.create_dataset("sine_wave", data=sine)
        signal_ds.attrs["units"] = "amplitude"
        signal_ds.attrs["SCALE"] = np.float32(1.25)
        signal_ds.attrs["OFFSET"] = np.float32(-0.10)
        signals.create_dataset("cosine_wave", data=cosine)
        signals.create_dataset("mixed_wave", data=mixed)
        signals.create_dataset(
            "parametric_xy",
            data=np.stack(
                [
                    np.sin(sample_axis * 0.75 + 0.3),
                    np.sin(sample_axis * 1.5),
                ],
                axis=-1,
            ).astype(np.float32),
        )

        group_preview = h5.create_group("group_preview")
        group_preview.attrs["scale"] = np.float32(1.4)
        group_preview.attrs["offset_attr"] = np.float32(0.2)
        group_preview.attrs["H5V_PREVIEW_EXPR"] = (
            "(!/group_preview/time, (!/group_preview/value - #/group_preview/offset) * #/group_preview:scale)"
        )
        group_preview.create_dataset(
            "time", data=np.linspace(0.0, 6.0, 48, dtype=np.float32)
        )
        group_preview.create_dataset(
            "value",
            data=(
                0.35
                + 0.55 * np.sin(np.linspace(0.0, 6.0, 48, dtype=np.float32) * 1.5)
                + 0.08
                * np.cos(np.linspace(0.0, 6.0, 48, dtype=np.float32) * 3.0 + 0.25)
            ).astype(np.float32),
        )
        group_preview.create_dataset("offset", data=np.float32(0.35))

        matrices = h5.create_group("matrices")
        rows = np.linspace(-1.0, 1.0, 18, dtype=np.float32)[:, None]
        cols = np.linspace(-1.0, 1.0, 24, dtype=np.float32)[None, :]
        heatmap = np.sin(rows * 4.0) + np.cos(cols * 6.0)
        matrices.create_dataset("heatmap", data=heatmap.astype(np.float32))
        cube = np.stack(
            [
                (heatmap + layer * 0.35).astype(np.float32)
                for layer in np.linspace(0.0, 1.0, 4, dtype=np.float32)
            ],
            axis=0,
        )
        matrices.create_dataset("cube", data=cube)

        strings = h5.create_group("strings")
        config = strings.create_dataset(
            "config_json",
            data='{"gain": 1.25, "offset": -0.1, "channels": ["A", "B"], "mode": "demo"}',
            dtype=utf8_dtype(),
        )
        config.attrs["HIGHLIGHT"] = "json"
        strings.create_dataset(
            "messages",
            data=np.array(["alpha", "beta", "gamma"], dtype=utf8_dtype()),
            dtype=utf8_dtype(),
        )
        strings.create_dataset(
            "pipeline.yml",
            data="""name: demo-pipeline
steps:
  - id: prepare
    action: normalize
  - id: render
    action: preview
settings:
  gain: 1.25
  offset: -0.10
""",
            dtype=utf8_dtype(),
        )
        strings.create_dataset(
            "demo.py",
            data="""import math


def scaled_wave(samples: int, gain: float = 1.25) -> list[float]:
    return [gain * math.sin(idx / 8.0) for idx in range(samples)]


print(scaled_wave(8))
""",
            dtype=utf8_dtype(),
        )
        strings.create_dataset(
            "notes.txt",
            data="""The .py and .yml datasets intentionally omit HIGHLIGHT attributes.
They should still be useful as inline string/code examples in h5v.
""",
            dtype=utf8_dtype(),
        )

        images = h5.create_group("images")
        truecolor = images.create_dataset(
            "truecolor_rgb", data=make_rgb_image(28, 36, phase=0.15)
        )
        image_attrs(truecolor, "IMAGE_TRUECOLOR", "INTERLACE_PIXEL")

        wide = images.create_dataset(
            "wide_grayscale", data=make_grayscale_image(32, 320, frequency=22.0)
        )
        image_attrs(wide, "IMAGE_GRAYSCALE")

        bitmap = images.create_dataset(
            "bitmap_mask",
            data=(make_grayscale_image(24, 24, frequency=8.0) > 127).astype(np.uint8),
        )
        image_attrs(bitmap, "IMAGE_BITMAP")

        raw_frames = images.create_dataset("varlen_png_frames", shape=(2,), dtype=vlen_u8)
        image_attrs(raw_frames, "IMAGE_PNG")
        raw_frames[0] = encode_image_bytes(make_rgb_image(20, 28, 0.0), "PNG")
        raw_frames[1] = encode_image_bytes(make_rgb_image(20, 28, 0.45), "PNG")

        compounds = h5.create_group("compound")
        record_dtype = np.dtype(
            [
                ("index", np.int16),
                ("amplitude", np.float32),
                ("valid", np.bool_),
                ("label", "S8"),
            ]
        )
        compounds.create_dataset(
            "records",
            data=np.array(
                [
                    (0, 0.25, True, b"start"),
                    (1, 0.72, True, b"peak"),
                    (2, -0.18, False, b"drop"),
                ],
                dtype=record_dtype,
            ),
        )
        nested_dtype = np.dtype(
            [
                ("meta", [("name", "S8"), ("enabled", np.bool_)]),
                ("window", np.int16, (3,)),
                ("gain", np.float64),
            ]
        )
        compounds.create_dataset(
            "nested_records",
            data=np.array(
                [
                    ((b"alpha", True), [0, 1, 2], 1.25),
                    ((b"beta", False), [2, 3, 4], 0.85),
                ],
                dtype=nested_dtype,
            ),
        )

        enums = h5.create_group("enums")
        enums.create_dataset(
            "quality",
            data=np.array([0, 1, 2, 1, 0, 2], dtype=np.uint8),
            dtype=enum_dtype,
        )

        metadata = h5.create_group("metadata")
        attributes_demo = metadata.create_dataset(
            "attributes_demo",
            data=np.arange(12, dtype=np.int16).reshape(3, 4),
        )
        attributes_demo.attrs["title"] = "Bundled example dataset"
        attributes_demo.attrs["scale"] = np.float32(1.25)
        attributes_demo.attrs["offset"] = np.float32(-0.10)
        attributes_demo.attrs["enabled"] = np.bool_(True)
        attributes_demo.attrs["bytes"] = np.bytes_(b"demo-bytes")
        attributes_demo.attrs["int_array"] = np.array([1, 2, 3], dtype=np.int32)
        attributes_demo.attrs["float_array"] = np.array([0.5, 1.5, 2.5], dtype=np.float32)
        attributes_demo.attrs["bool_array"] = np.array([True, False, True], dtype=np.bool_)
        attributes_demo.attrs["labels"] = np.array(
            ["alpha", "beta"], dtype=utf8_dtype()
        )
        attr_compound_dtype = np.dtype([("gain", np.float32), ("name", "S8")])
        attributes_demo.attrs["compound_scalar"] = np.array(
            (1.5, b"demo"), dtype=attr_compound_dtype
        )

        references = h5.create_group("references")
        target_group = references.create_group("targets")
        target_dataset = target_group.create_dataset(
            "dataset_target", data=np.arange(6, dtype=np.int32)
        )
        object_probe = references.create_dataset(
            "object_attr_probe", data=np.arange(3, dtype=np.uint8)
        )
        object_probe.attrs["dataset_ref"] = target_dataset.ref
        object_probe.attrs["group_ref"] = target_group.ref
        attributes_demo.attrs["dataset_ref"] = signal_ds.ref
        attributes_demo.attrs["group_ref"] = images.ref

        links = h5.create_group("links")
        links["hard_link_to_sine"] = signal_ds
        links["soft_link_to_wide_image"] = h5py.SoftLink("/images/wide_grayscale")


def main() -> None:
    build_example_file(OUTPUT_PATH)
    print(f"Wrote {OUTPUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
