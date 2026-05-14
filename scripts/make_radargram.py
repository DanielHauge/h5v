#!/usr/bin/env python3

"""
Author+Credits: ChatGPT5 lol - just needed ar radargram for testing
Generate a VERY LARGE synthetic radargram and store it in HDF5.

This version is designed for:
- Huge datasets (> 50,000 traces)
- Streaming/chunked HDF5 writing
- Low memory usage
- Realistic radargram appearance
- Fast generation

Example output shape:
    (512 samples, 60000 traces)

Dependencies:
    pip install numpy matplotlib h5py
"""

import numpy as np
import h5py
import matplotlib.pyplot as plt


# =============================================================================
# CONFIG
# =============================================================================

NUM_TRACES = 60000  # SUPER LONG DIRECTION
NUM_SAMPLES = 512

OUTPUT_H5 = "huge_radargram.h5"
PREVIEW_IMAGE = "huge_radargram_preview.png"

CHUNK_TRACES = 512  # HDF5 chunk size

np.random.seed(1337)


# =============================================================================
# WAVELET
# =============================================================================


def ricker_wavelet(length=41, sigma=5.0):
    x = np.arange(length) - length // 2

    wavelet = (1 - (x**2 / sigma**2)) * np.exp(-(x**2) / (2 * sigma**2))

    wavelet /= np.max(np.abs(wavelet))

    return wavelet.astype(np.float32)


WAVELET = ricker_wavelet()


# =============================================================================
# SIGNAL HELPERS
# =============================================================================


def add_wavelet(trace, center, amplitude):
    """
    Add a wavelet into a single trace.
    """

    half = len(WAVELET) // 2

    start = center - half
    end = start + len(WAVELET)

    w0 = 0
    w1 = len(WAVELET)

    if start < 0:
        w0 = -start
        start = 0

    if end > len(trace):
        w1 -= end - len(trace)
        end = len(trace)

    if start < end:
        trace[start:end] += amplitude * WAVELET[w0:w1]


# =============================================================================
# GENERATE SINGLE TRACE
# =============================================================================


def generate_trace(x):
    """
    Generate one synthetic radar trace.
    """

    trace = np.zeros(NUM_SAMPLES, dtype=np.float32)

    # -------------------------------------------------------------------------
    # Layer 1
    # -------------------------------------------------------------------------

    y1 = 60 + 8 * np.sin(x * 0.0008) + 5 * np.sin(x * 0.003)

    add_wavelet(trace, int(y1), 1.0)

    # -------------------------------------------------------------------------
    # Layer 2
    # -------------------------------------------------------------------------

    y2 = 150 + 15 * np.sin(x * 0.0003 + 1.5) + 6 * np.sin(x * 0.001)

    add_wavelet(trace, int(y2), -0.8)

    # -------------------------------------------------------------------------
    # Layer 3
    # -------------------------------------------------------------------------

    y3 = 260 + 18 * np.sin(x * 0.0002 + 0.3)

    add_wavelet(trace, int(y3), 0.5)

    # -------------------------------------------------------------------------
    # Hyperbolic Targets
    # -------------------------------------------------------------------------

    hyperbolas = [
        (7000, 90, 20, 1.6),
        (18000, 120, 35, -1.4),
        (32000, 180, 25, 1.2),
        (47000, 100, 18, 1.0),
        (55000, 140, 40, -1.1),
    ]

    for cx, apex, width, amp in hyperbolas:
        dx = (x - cx) / width

        y = int(apex + 25 * np.sqrt(1 + dx * dx))

        if y < NUM_SAMPLES:
            add_wavelet(trace, y, amp)

    # -------------------------------------------------------------------------
    # Depth attenuation
    # -------------------------------------------------------------------------

    attenuation = np.linspace(1.0, 0.25, NUM_SAMPLES)

    trace *= attenuation

    # -------------------------------------------------------------------------
    # Time gain
    # -------------------------------------------------------------------------

    gain = np.linspace(1.0, 2.0, NUM_SAMPLES)

    trace *= gain

    # -------------------------------------------------------------------------
    # Noise
    # -------------------------------------------------------------------------

    trace += np.random.normal(
        0,
        0.05,
        NUM_SAMPLES,
    ).astype(np.float32)

    return trace


# =============================================================================
# MAIN
# =============================================================================

print("Creating HDF5 dataset...")

with h5py.File(OUTPUT_H5, "w") as f:
    dataset = f.create_dataset(
        "radargram",
        shape=(NUM_SAMPLES, NUM_TRACES),
        dtype=np.float32,
        compression="gzip",
        chunks=(NUM_SAMPLES, CHUNK_TRACES),
    )

    # Metadata
    dataset.attrs["description"] = "Huge synthetic radargram"
    dataset.attrs["num_samples"] = NUM_SAMPLES
    dataset.attrs["num_traces"] = NUM_TRACES

    # -------------------------------------------------------------------------
    # Generate in chunks
    # -------------------------------------------------------------------------

    for start in range(0, NUM_TRACES, CHUNK_TRACES):
        end = min(start + CHUNK_TRACES, NUM_TRACES)

        width = end - start

        block = np.zeros(
            (NUM_SAMPLES, width),
            dtype=np.float32,
        )

        for i, x in enumerate(range(start, end)):
            block[:, i] = generate_trace(x)

        # Slight horizontal coherence
        for i in range(1, width):
            block[:, i] += 0.15 * block[:, i - 1]

        # Normalize block
        block /= np.max(np.abs(block))

        dataset[:, start:end] = block

        print(f"Generated traces {start:6d} -> {end:6d}")

print(f"\nSaved HDF5 file: {OUTPUT_H5}")


# =============================================================================
# PREVIEW IMAGE
# =============================================================================

print("Generating preview image...")

with h5py.File(OUTPUT_H5, "r") as f:
    data = f["radargram"]

    # Downsample preview
    preview = data[:, ::60]

    plt.figure(figsize=(18, 6))

    plt.imshow(
        preview,
        cmap="gray",
        aspect="auto",
        interpolation="nearest",
    )

    plt.title("Huge Synthetic Radargram Preview")
    plt.xlabel("Trace (downsampled)")
    plt.ylabel("Time Sample")

    plt.tight_layout()

    plt.savefig(
        PREVIEW_IMAGE,
        dpi=200,
    )

print(f"Saved preview image: {PREVIEW_IMAGE}")
