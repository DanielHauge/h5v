# Introduction

h5v is a terminal user interface for inspecting, visualizing, and editing HDF5 files. It is designed for workflows where you want the structure of the file, the current data slice, and the surrounding metadata visible at the same time instead of bouncing between scripts and one-off viewers.

![h5v interface overview](./assets/help.png)

At a high level, h5v combines:

- a navigable HDF5 tree for groups, datasets, links, and projected compound fields
- preview modes for numeric series, dense matrices, scalar values, strings, and inline image datasets
- an attribute pane for inspecting and editing metadata
- a command layer for reproducible navigation and scripted startup automation
- a multichart workspace for comparing several series at once

The rest of this book is split on purpose:

- use the first few chapters to get installed and productive quickly
- use the middle chapters as the reference for preview behavior, controls, and supported HDF5 structures
- use the multichart and command chapters when you want repeatable or analysis-heavy workflows

If you only need to get moving, start with [Installation](./installation.md) and [Quick start](./quick-start.md).
