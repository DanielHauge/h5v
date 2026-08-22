#!/usr/bin/env python3

from pathlib import Path

import h5py
import numpy as np


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = ROOT / "examples" / "quick-demo.h5"


def build_quick_demo(output_path: Path) -> None:
    rng = np.random.default_rng(20260822)
    depth = np.linspace(0.0, 1.0, 128, dtype=np.float32)[:, None]
    trace = np.linspace(0.0, 1.0, 256, dtype=np.float32)[None, :]
    reflector_a = np.sin(70.0 * (depth - 0.22 - 0.06 * np.sin(trace * 2.0 * np.pi)))
    reflector_b = np.sin(55.0 * (depth - 0.58 - 0.10 * np.cos(trace * 3.0 * np.pi)))
    attenuation = np.exp(-depth * 2.5)
    radargram = (attenuation * (reflector_a + 0.65 * reflector_b) + rng.normal(
        0.0, 0.08, size=(128, 256)
    )).astype(np.float32)
    samples = np.linspace(0.0, 4.0 * np.pi, 256, dtype=np.float32)
    noisy_sine = (np.sin(samples) + rng.normal(0.0, 0.1, size=256)).astype(np.float32)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with h5py.File(output_path, "w") as h5:
        demo = h5.create_group("demo")
        demo.attrs["description"] = "Small deterministic radar and signal demo."
        heatmap = demo.create_dataset("radargram", data=radargram)
        heatmap.attrs["description"] = "Synthetic attenuated reflection heatmap."
        heatmap.attrs["units"] = "amplitude"
        signal = demo.create_dataset("noisy_sine", data=noisy_sine)
        signal.attrs["description"] = "Noisy sinusoidal signal."
        signal.attrs["units"] = "amplitude"


def main() -> None:
    build_quick_demo(OUTPUT_PATH)
    print(f"Wrote {OUTPUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
