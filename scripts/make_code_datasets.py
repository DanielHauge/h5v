import h5py
import numpy as np

try:
    import os

    os.remove("code.h5")
except OSError:
    pass

with h5py.File("code.h5", "w") as f:
    # some random_data made into json string
    data = {
        "random_data": np.random.rand(10).tolist(),
        "info": "This is some random data",
        "version": 1.0,
    }
    # simple conversion to JSON-like string
    json_str = str(data).replace("'", '"')
    print(json_str)
    js_dtype = h5py.string_dtype(encoding="utf-8")
    ds = f.create_dataset("json_data", data=json_str, dtype=js_dtype)
    ds.attrs["HIGHLIGHT"] = "json"
    # how about some yaml too, just use the same data
    try:
        import yaml

        yaml_str = yaml.dump(data)
        y_dtype = h5py.string_dtype(encoding="utf-8")
        ds2 = f.create_dataset("yaml_data", data=yaml_str, dtype=y_dtype)
        ds2.attrs["HIGHLIGHT"] = "yaml"
    except ImportError:
        yaml_str = "yaml module not installed"

    # take this python code and store it as a dataset
    with open(__file__, "r") as code_file:
        code_content = code_file.read()
        code_dtype = h5py.string_dtype(encoding="utf-8")
        ds3 = f.create_dataset("python", data=code_content, dtype=code_dtype)
        ds3.attrs["HIGHLIGHT"] = "py"

    # lets also do some rhai code
    rhai_code = """
// This is some Rhai code
fn factorial(n) {
    if n <= 1 {
        return 1;
    } else {
        return n * factorial(n - 1);
    }
}
let result = factorial(5);
print(result);
"""
    rhai_dtype = h5py.string_dtype(encoding="utf-8")
    ds4 = f.create_dataset("rhai", data=rhai_code, dtype=rhai_dtype)
    ds4.attrs["HIGHLIGHT"] = "rs"

    averylongstring_lorem_ipsum = (
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore "
        "et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea "
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. "
        "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."
        * 5
    )
    verylong5timesseperatedbynewlines = "\n".join([averylongstring_lorem_ipsum] * 5)
    longstr_dtype = h5py.string_dtype(
        encoding="utf-8", length=len(verylong5timesseperatedbynewlines)
    )
    ds5 = f.create_dataset(
        "long_string", data=verylong5timesseperatedbynewlines, dtype=longstr_dtype
    )
