# Scripting

h5v supports custom plotting previews based on rhai scripts.

## Example

```rust

// A script could be evaluated with a dataset in context.
let dataset = context();
let dataset_name = dataset.path();
let parent_name = dataset.split('/').skip_last().join("/");

// Load an attribute as a constant
let scale = attribute_as_f64(parent_name, "SCALE");
// Load a dataset.
let dataset = dataset("/path/to/dataset");
// Perform a transformation
let y = dataset * scale;

// Create a new plot
let plot = new_line();
// let plot = new_scatter();
// let plot = new_histogram();

// Add some data to it, will automatically generate x-axis 0..len(y)
plot.add_data(y);

// Various things can be done with the dataset like get length
let y_len = y.len();
// Generate linear space for x-axis maybe?
let x = linspace(0.0, 10.0, y_len);
// Add data with custom x-axis
plot.add_data(x, y);
plot.set_title("My Plot");
plot.set_x_label("X-axis");
plot.set_y_label("Y-axis");
plot.set_legend(vec!["Data 1", "Data 2"]);
// Evaluate to a plot struct, this will plot the stuff.
plot
```

## Configuring for plot script

A way to customize a plot using a script. In the script many datasets and attributes can be loaded, and calculated to create a nice plot with various settings etc.

PLOT_SCRIPT: "/path/to/script_ds"
PLOT_TITLE: "My Plot"
PLOT_XLABEL: "X-axis"
PLOT_YLABEL: "Y-axis"
PLOT_LEGEND: ["Data 1", "Data 2"]

Could point to self, if just a plot that fetches other datasets, and doesn't need to be plotted with a context.
