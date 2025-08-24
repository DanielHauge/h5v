# Scripting

```rust
// Load an attribute as a constant
let scale = attribute_as_f64("/path/to/attribute", "SCALE");
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
plot.size(800, 600);
// Evaluate to a plot struct, this will plot the stuff.
plot
```

# Setting script

PREVIEW_SCRIPT: "let hejsa = 42;"
PREVIEW_SCRIPT_DS: "/path/to/script_ds"
