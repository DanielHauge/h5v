import h5py
import numpy as np

# make a file

with h5py.File("highdim.h5", "w") as f:
    # make 3 dimensions
    # first dimension is A
    # second dimension is B
    # third dimension is C
    # Data should then be: # A*100 + B*10 + C
    A = 100
    B = 9
    C = 5
    data = np.zeros((A, B, C), dtype=np.int32)
    for a in range(A):
        for b in range(B):
            for c in range(C):
                data[a, b, c] = a * 100 + b * 10 + c

    f.create_dataset("my_data", data=data)
