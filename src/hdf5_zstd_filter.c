/*
 * HDF5 Zstandard filter, filter ID 32015.
 *
 * Adapted from H5Zzstd.c in netCDF-C.
 * Copyright (c) 2018-2024 Unidata. All rights reserved.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 * 3. Neither the name of the copyright holder nor the names of its contributors
 *    may be used to endorse or promote products derived from this software
 *    without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
 * LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES.
 */

#include <limits.h>
#include <stddef.h>
#include <stdint.h>

#include "hdf5.h"
#include "zstd.h"

#define H5V_ZSTD_FILTER_ID 32015

static size_t
h5v_zstd_filter(unsigned int flags, size_t cd_nelmts,
                const unsigned int cd_values[], size_t nbytes,
                size_t *buf_size, void **buf)
{
    void *output;
    size_t output_size;
    size_t result;

    if (cd_nelmts != 1)
        return 0;

    if (flags & H5Z_FLAG_REVERSE) {
        unsigned long long frame_size = ZSTD_getFrameContentSize(*buf, nbytes);

        if (frame_size == ZSTD_CONTENTSIZE_ERROR ||
            frame_size == ZSTD_CONTENTSIZE_UNKNOWN || frame_size > SIZE_MAX)
            return 0;
        output_size = (size_t)frame_size;
        output = H5allocate_memory(output_size, 0);
        if (output == NULL)
            return 0;
        result = ZSTD_decompress(output, output_size, *buf, nbytes);
    } else {
        output_size = ZSTD_compressBound(nbytes);
        output = H5allocate_memory(output_size, 0);
        if (output == NULL)
            return 0;
        result = ZSTD_compress(output, output_size, *buf, nbytes, (int)cd_values[0]);
    }

    if (ZSTD_isError(result)) {
        H5free_memory(output);
        return 0;
    }

    H5free_memory(*buf);
    *buf = output;
    *buf_size = output_size;
    return result;
}

static const H5Z_class2_t h5v_zstd_filter_class = {
    H5Z_CLASS_T_VERS,
    H5V_ZSTD_FILTER_ID,
    1,
    1,
    "Zstandard",
    NULL,
    NULL,
    h5v_zstd_filter,
};

int
h5v_register_hdf5_zstd_filter(void)
{
    return H5Zregister(&h5v_zstd_filter_class) < 0 ? -1 : 0;
}
