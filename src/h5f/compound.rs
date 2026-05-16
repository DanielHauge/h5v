use std::{ffi::CStr, mem::size_of};

use hdf5_metno::{
    h5check,
    types::{CompoundType, FloatSize, IntSize, TypeDescriptor},
    Dataset, Dataspace, Datatype, Selection,
};
use hdf5_metno_sys::{h5p::H5P_DEFAULT, h5t::H5Treclaim};
use ndarray::{Array1, Array2};

use crate::{
    data::{
        plot_sampling_step_with_cap, validate_preview_selection_shape, DatasetPlotingData,
        PreviewSelection, SliceSelection,
    },
    error::AppError,
};

use super::meta::{CompoundFieldProjection, DatasetMeta};

unsafe extern "C" {
    fn H5Dread(
        dset_id: i64,
        mem_type_id: i64,
        mem_space_id: i64,
        file_space_id: i64,
        plist_id: i64,
        buf: *mut std::ffi::c_void,
    ) -> i32;
    fn H5Dwrite(
        dset_id: i64,
        mem_type_id: i64,
        mem_space_id: i64,
        file_space_id: i64,
        plist_id: i64,
        buf: *const std::ffi::c_void,
    ) -> i32;
}

const H5_DEFAULT_ID: i64 = 0;

fn checked_byte_len(
    item_size: usize,
    total_elems: usize,
    context: &str,
) -> Result<usize, AppError> {
    item_size
        .checked_mul(total_elems)
        .ok_or_else(|| AppError::DrawingError(format!("{context} byte size overflowed usize")))
}

pub fn compound_children(meta: &DatasetMeta) -> Option<Vec<CompoundFieldProjection>> {
    let projection = meta.compound_projection.as_ref()?;
    let compound = projection.current_compound_type()?;
    Some(
        compound
            .fields
            .iter()
            .map(|field| projection.child(field))
            .collect(),
    )
}

fn selection_to_shape(selection: &Selection, dataset: &Dataset) -> Result<Vec<usize>, AppError> {
    let raw = selection.clone().into_raw(dataset.shape())?;
    let resolved = Selection::from_raw(raw)?;
    Ok(resolved.out_shape(dataset.shape())?)
}

fn selection_mem_shape(shape: &[usize]) -> Vec<usize> {
    if shape.is_empty() {
        vec![1]
    } else {
        shape.to_vec()
    }
}

fn read_projected_bytes(
    dataset: &Dataset,
    projection: &CompoundFieldProjection,
    selection: Selection,
) -> Result<(Vec<u8>, Vec<usize>), AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let out_shape = selection_to_shape(&selection, dataset)?;
    let total_elems = out_shape.iter().product::<usize>();
    let buffer_len = checked_byte_len(item_size, total_elems, "Projected compound selection")?;

    let file_space = dataset.space()?.copy();
    let raw_selection = selection.into_raw(dataset.shape())?;
    unsafe {
        raw_selection.apply_to_dataspace(file_space.id())?;
    }
    let mem_space = Dataspace::try_new(selection_mem_shape(&out_shape))?;

    let mut buffer = vec![0_u8; buffer_len];
    let status = unsafe {
        H5Dread(
            dataset.id(),
            dtype.id(),
            mem_space.id(),
            file_space.id(),
            H5_DEFAULT_ID,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::DrawingError(
            "Failed reading projected compound selection".to_string(),
        ));
    }

    let field_size = projection.field_type.size();
    let start_offset = projection.absolute_offset();
    let out_capacity = checked_byte_len(field_size, total_elems, "Projected compound field")?;
    let mut out = Vec::with_capacity(out_capacity);
    for chunk in buffer.chunks_exact(item_size) {
        let end = start_offset + field_size;
        if end > chunk.len() {
            return Err(AppError::DrawingError(
                "Compound projection exceeded element bounds".to_string(),
            ));
        }
        out.extend_from_slice(&chunk[start_offset..end]);
    }
    Ok((out, out_shape))
}

fn read_projected_scalar_bytes(
    dataset: &Dataset,
    projection: &CompoundFieldProjection,
) -> Result<Vec<u8>, AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let mut buffer = vec![0_u8; item_size];
    let status = unsafe {
        H5Dread(
            dataset.id(),
            dtype.id(),
            H5_DEFAULT_ID,
            H5_DEFAULT_ID,
            H5_DEFAULT_ID,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::DrawingError(
            "Failed reading projected compound scalar".to_string(),
        ));
    }

    let field_size = projection.field_type.size();
    let start = projection.absolute_offset();
    let end = start + field_size;
    if end > buffer.len() {
        return Err(AppError::DrawingError(
            "Compound scalar projection exceeded element bounds".to_string(),
        ));
    }
    Ok(buffer[start..end].to_vec())
}

pub fn read_selected_element_bytes(
    dataset: &Dataset,
    selection: Option<&Selection>,
) -> Result<Vec<u8>, AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let mut buffer = vec![0_u8; item_size];

    let (mem_space, file_space, mem_space_id, file_space_id) = if let Some(selection) = selection {
        let selected_file_space = dataset.space()?.copy();
        let raw_selection = selection.clone().into_raw(dataset.shape())?;
        unsafe {
            raw_selection.apply_to_dataspace(selected_file_space.id())?;
        }
        let mem_shape = selection_to_shape(selection, dataset)?;
        let selected_mem_space = Dataspace::try_new(selection_mem_shape(&mem_shape))?;
        let mem_space_id = selected_mem_space.id();
        let file_space_id = selected_file_space.id();
        (
            Some(selected_mem_space),
            Some(selected_file_space),
            mem_space_id,
            file_space_id,
        )
    } else {
        (None, None, H5_DEFAULT_ID, H5_DEFAULT_ID)
    };
    let _keep_alive = (mem_space.as_ref(), file_space.as_ref());

    let status = unsafe {
        H5Dread(
            dataset.id(),
            dtype.id(),
            mem_space_id,
            file_space_id,
            H5_DEFAULT_ID,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::DrawingError(
            "Failed reading selected dataset element".to_string(),
        ));
    }

    Ok(buffer)
}

pub fn read_dataset_raw_bytes(dataset: &Dataset) -> Result<Vec<u8>, AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let total_elems = dataset.size();
    let total_bytes = checked_byte_len(item_size, total_elems, "Dataset")?;
    let mut buffer = vec![0_u8; total_bytes];

    let status = unsafe {
        H5Dread(
            dataset.id(),
            dtype.id(),
            H5_DEFAULT_ID,
            H5_DEFAULT_ID,
            H5_DEFAULT_ID,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::DrawingError(
            "Failed reading raw dataset bytes".to_string(),
        ));
    }

    Ok(buffer)
}

pub fn read_selected_values_bytes(
    dataset: &Dataset,
    selection: Selection,
) -> Result<(Vec<u8>, Vec<usize>), AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let out_shape = selection_to_shape(&selection, dataset)?;
    let total_elems = out_shape.iter().product::<usize>();
    let buffer_len = checked_byte_len(item_size, total_elems, "Selected dataset values")?;

    let file_space = dataset.space()?.copy();
    let raw_selection = selection.into_raw(dataset.shape())?;
    unsafe {
        raw_selection.apply_to_dataspace(file_space.id())?;
    }
    let mem_space = Dataspace::try_new(selection_mem_shape(&out_shape))?;

    let mut buffer = vec![0_u8; buffer_len];
    let status = unsafe {
        H5Dread(
            dataset.id(),
            dtype.id(),
            mem_space.id(),
            file_space.id(),
            H5_DEFAULT_ID,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::DrawingError(
            "Failed reading selected raw dataset bytes".to_string(),
        ));
    }

    Ok((buffer, out_shape))
}

pub fn write_selected_element_bytes(
    dataset: &Dataset,
    selection: Option<&Selection>,
    bytes: &[u8],
) -> Result<(), AppError> {
    let dtype = dataset.dtype()?;
    if bytes.len() != dtype.size() {
        return Err(AppError::EditError(format!(
            "Selected dataset element write size mismatch: expected {} bytes, got {}",
            dtype.size(),
            bytes.len()
        )));
    }

    let (mem_space, file_space, mem_space_id, file_space_id) = if let Some(selection) = selection {
        let selected_file_space = dataset.space()?.copy();
        let raw_selection = selection.clone().into_raw(dataset.shape())?;
        unsafe {
            raw_selection.apply_to_dataspace(selected_file_space.id())?;
        }
        let mem_shape = selection_to_shape(selection, dataset)?;
        let selected_mem_space = Dataspace::try_new(selection_mem_shape(&mem_shape))?;
        let mem_space_id = selected_mem_space.id();
        let file_space_id = selected_file_space.id();
        (
            Some(selected_mem_space),
            Some(selected_file_space),
            mem_space_id,
            file_space_id,
        )
    } else {
        (None, None, H5_DEFAULT_ID, H5_DEFAULT_ID)
    };
    let _keep_alive = (mem_space.as_ref(), file_space.as_ref());

    let status = unsafe {
        H5Dwrite(
            dataset.id(),
            dtype.id(),
            mem_space_id,
            file_space_id,
            H5_DEFAULT_ID,
            bytes.as_ptr().cast(),
        )
    };
    if status < 0 {
        return Err(AppError::EditError(
            "Failed writing selected dataset element".to_string(),
        ));
    }

    Ok(())
}

pub fn read_projected_selection_bytes(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Option<&Selection>,
) -> Result<Vec<u8>, AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::DrawingError("Projected bytes requested for non-compound dataset".to_string())
    })?;
    if selection.is_none() && dataset.is_scalar() {
        return read_projected_scalar_bytes(dataset, projection);
    }

    let selection = selection.ok_or_else(|| {
        AppError::DrawingError("Missing selection for projected dataset element".to_string())
    })?;
    let (bytes, out_shape) = read_projected_bytes(dataset, projection, selection.clone())?;
    let selected_elems = if out_shape.is_empty() {
        1
    } else {
        out_shape.iter().product()
    };
    if selected_elems != 1 {
        return Err(AppError::DrawingError(format!(
            "Expected scalar projected selection, got shape {:?}",
            out_shape
        )));
    }
    Ok(bytes)
}

pub fn write_projected_selection_bytes(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Option<&Selection>,
    field_bytes: &[u8],
) -> Result<(), AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::EditError("Projected write requested for non-compound dataset".to_string())
    })?;
    let field_size = projection.field_type.size();
    if field_bytes.len() != field_size {
        return Err(AppError::EditError(format!(
            "Projected field write size mismatch: expected {} bytes, got {}",
            field_size,
            field_bytes.len()
        )));
    }

    let mut element_bytes = read_selected_element_bytes(dataset, selection)?;
    let start = projection.absolute_offset();
    let end = start + field_size;
    if end > element_bytes.len() {
        return Err(AppError::EditError(
            "Projected field write exceeded element bounds".to_string(),
        ));
    }
    element_bytes[start..end].copy_from_slice(field_bytes);
    write_selected_element_bytes(dataset, selection, &element_bytes)
}

pub trait ProjectionDecode: Sized {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError>;

    fn decode_scalar_buffer(
        field_type: &TypeDescriptor,
        bytes: &mut [u8],
    ) -> Result<Self, AppError> {
        let result = Self::decode(field_type, bytes);
        let reclaim_result = reclaim_projected_varlen_if_needed(field_type, &[], bytes);
        match (result, reclaim_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(err), _) => Err(err),
            (_, Err(err)) => Err(err),
        }
    }

    fn decode_value_buffer(
        field_type: &TypeDescriptor,
        out_shape: &[usize],
        bytes: &mut [u8],
    ) -> Result<Vec<Self>, AppError> {
        let field_size = field_type.size();
        let result = bytes
            .chunks_exact(field_size)
            .map(|chunk| Self::decode(field_type, chunk))
            .collect::<Result<Vec<_>, _>>();
        let reclaim_result = reclaim_projected_varlen_if_needed(field_type, out_shape, bytes);
        match (result, reclaim_result) {
            (Ok(values), Ok(())) => Ok(values),
            (Err(err), _) => Err(err),
            (_, Err(err)) => Err(err),
        }
    }
}

impl ProjectionDecode for f64 {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError> {
        match field_type {
            TypeDescriptor::Float(FloatSize::U4) => Ok(f32::from_le_bytes(to_array(bytes)?) as f64),
            TypeDescriptor::Float(FloatSize::U8) => Ok(f64::from_le_bytes(to_array(bytes)?)),
            TypeDescriptor::Integer(_) => Ok(decode_i64(field_type, bytes)? as f64),
            TypeDescriptor::Unsigned(_) | TypeDescriptor::Boolean | TypeDescriptor::Enum(_) => {
                Ok(decode_u64(field_type, bytes)? as f64)
            }
            _ => Err(AppError::DrawingError(format!(
                "Unsupported projected numeric type: {field_type}"
            ))),
        }
    }
}

impl ProjectionDecode for i64 {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError> {
        decode_i64(field_type, bytes)
    }
}

impl ProjectionDecode for u64 {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError> {
        decode_u64(field_type, bytes)
    }
}

impl ProjectionDecode for String {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError> {
        match field_type {
            TypeDescriptor::FixedArray(inner, size) => {
                let inner_size = inner.size();
                let values = bytes
                    .chunks_exact(inner_size)
                    .take(*size)
                    .map(|chunk| String::decode(inner, chunk))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("[{}]", values.join(", ")))
            }
            TypeDescriptor::Integer(_) => Ok(decode_i64(field_type, bytes)?.to_string()),
            TypeDescriptor::Unsigned(_) => Ok(decode_u64(field_type, bytes)?.to_string()),
            TypeDescriptor::Boolean => Ok((decode_u64(field_type, bytes)? != 0).to_string()),
            TypeDescriptor::Float(FloatSize::U4) => {
                Ok(f32::from_le_bytes(to_array(bytes)?).to_string())
            }
            TypeDescriptor::Float(FloatSize::U8) => {
                Ok(f64::from_le_bytes(to_array(bytes)?).to_string())
            }
            TypeDescriptor::Enum(enum_type) => {
                Ok(decode_u64(&TypeDescriptor::Enum(enum_type.clone()), bytes)?.to_string())
            }
            TypeDescriptor::FixedAscii(_) | TypeDescriptor::FixedUnicode(_) => {
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
            }
            TypeDescriptor::VarLenAscii => decode_projected_varlen_ascii(bytes),
            TypeDescriptor::VarLenUnicode => decode_projected_varlen_unicode(bytes),
            _ => Err(AppError::DrawingError(format!(
                "Unsupported projected string type: {field_type}"
            ))),
        }
    }
}

pub fn read_projected_scalar<T: ProjectionDecode>(
    dataset: &Dataset,
    meta: &DatasetMeta,
) -> Result<T, AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::DrawingError("Projected scalar requested for non-compound dataset".to_string())
    })?;
    let mut bytes = read_projected_scalar_bytes(dataset, projection)?;
    T::decode_scalar_buffer(&projection.field_type, &mut bytes)
}

pub fn read_projected_values_1d<T: ProjectionDecode>(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Selection,
) -> Result<Array1<T>, AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::DrawingError("Projected values requested for non-compound dataset".to_string())
    })?;
    let (mut bytes, out_shape) = read_projected_bytes(dataset, projection, selection)?;
    if out_shape.len() != 1 {
        return Err(AppError::DrawingError(format!(
            "Expected 1D projected shape, got {:?}",
            out_shape
        )));
    }
    let values = T::decode_value_buffer(&projection.field_type, &out_shape, &mut bytes)?;
    Ok(Array1::from(values))
}

pub fn read_projected_values_2d<T: ProjectionDecode>(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Selection,
) -> Result<Array2<T>, AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::DrawingError("Projected values requested for non-compound dataset".to_string())
    })?;
    let (mut bytes, out_shape) = read_projected_bytes(dataset, projection, selection)?;
    if out_shape.len() != 2 {
        return Err(AppError::DrawingError(format!(
            "Expected 2D projected shape, got {:?}",
            out_shape
        )));
    }
    let values = T::decode_value_buffer(&projection.field_type, &out_shape, &mut bytes)?;
    Array2::from_shape_vec((out_shape[0], out_shape[1]), values).map_err(|e| {
        AppError::DrawingError(format!("Failed shaping projected 2D field values: {e}"))
    })
}

pub fn plot_projected(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: &PreviewSelection,
) -> Result<DatasetPlotingData, AppError> {
    plot_projected_with_cap(dataset, meta, selection, usize::MAX)
}

pub fn plot_projected_with_cap(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: &PreviewSelection,
    max_samples: usize,
) -> Result<DatasetPlotingData, AppError> {
    let shape = dataset.shape();
    validate_preview_selection_shape(&shape, selection).map_err(AppError::Hdf5)?;
    let slice = match selection.slice {
        SliceSelection::All => 0..shape[selection.x],
        SliceSelection::FromTo(a, b) => a..b,
    };
    let length = slice.end.saturating_sub(slice.start);
    let step = plot_sampling_step_with_cap(length, max_samples);
    let mut slice_selections = Vec::new();
    for idx in 0..shape.len() {
        if idx == selection.x {
            slice_selections.push(hdf5_metno::SliceOrIndex::SliceTo {
                start: slice.start,
                step,
                end: slice.end,
                block: 1,
            });
        } else {
            slice_selections.push(hdf5_metno::SliceOrIndex::Index(selection.index[idx]));
        }
    }
    let selection = Selection::Hyperslab(hdf5_metno::Hyperslab::from(slice_selections));
    let values = read_projected_values_1d::<f64>(dataset, meta, selection)?;
    let data = values
        .iter()
        .enumerate()
        .map(|(idx, value)| ((idx * step) as f64, *value))
        .collect::<Vec<_>>();
    let max = data.iter().map(|(_, y)| *y).fold(f64::NAN, f64::max);
    let min = data.iter().map(|(_, y)| *y).fold(f64::NAN, f64::min);
    Ok(DatasetPlotingData {
        data,
        length,
        max,
        min,
    })
}

fn decode_i64(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<i64, AppError> {
    match field_type {
        TypeDescriptor::Integer(IntSize::U1) => Ok(i8::from_le_bytes(to_array(bytes)?) as i64),
        TypeDescriptor::Integer(IntSize::U2) => Ok(i16::from_le_bytes(to_array(bytes)?) as i64),
        TypeDescriptor::Integer(IntSize::U4) => Ok(i32::from_le_bytes(to_array(bytes)?) as i64),
        TypeDescriptor::Integer(IntSize::U8) => Ok(i64::from_le_bytes(to_array(bytes)?)),
        _ => Err(AppError::DrawingError(format!(
            "Unsupported projected signed type: {field_type}"
        ))),
    }
}

fn decode_u64(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<u64, AppError> {
    match field_type {
        TypeDescriptor::Unsigned(IntSize::U1) => Ok(u8::from_le_bytes(to_array(bytes)?) as u64),
        TypeDescriptor::Unsigned(IntSize::U2) => Ok(u16::from_le_bytes(to_array(bytes)?) as u64),
        TypeDescriptor::Unsigned(IntSize::U4) => Ok(u32::from_le_bytes(to_array(bytes)?) as u64),
        TypeDescriptor::Unsigned(IntSize::U8) => Ok(u64::from_le_bytes(to_array(bytes)?)),
        TypeDescriptor::Boolean => Ok(u8::from_le_bytes(to_array(bytes)?) as u64),
        TypeDescriptor::Enum(enum_type) => {
            let base = enum_type.base_type();
            match base {
                TypeDescriptor::Integer(_) => Ok(decode_i64(&base, bytes)? as u64),
                TypeDescriptor::Unsigned(_) => decode_u64(&base, bytes),
                _ => Err(AppError::DrawingError(format!(
                    "Unsupported projected enum base type: {base}"
                ))),
            }
        }
        _ => Err(AppError::DrawingError(format!(
            "Unsupported projected unsigned type: {field_type}"
        ))),
    }
}

fn to_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], AppError> {
    bytes.try_into().map_err(|_| {
        AppError::DrawingError(format!(
            "Failed converting {} bytes into fixed array of {} bytes",
            bytes.len(),
            N
        ))
    })
}

fn decode_projected_varlen_ptr(bytes: &[u8]) -> Result<*mut u8, AppError> {
    if bytes.len() != size_of::<*mut u8>() {
        return Err(AppError::DrawingError(format!(
            "Failed decoding projected varlen string pointer: expected {} bytes, got {}",
            size_of::<*mut u8>(),
            bytes.len()
        )));
    }
    Ok(usize::from_ne_bytes(to_array::<{ size_of::<usize>() }>(bytes)?) as *mut u8)
}

fn decode_projected_varlen_ascii(bytes: &[u8]) -> Result<String, AppError> {
    let ptr = decode_projected_varlen_ptr(bytes)?;
    if ptr.is_null() {
        return Ok(String::new());
    }
    let bytes = unsafe { CStr::from_ptr(ptr.cast()) }.to_bytes();
    if !bytes.is_ascii() {
        return Err(AppError::DrawingError(
            "Projected ASCII string contained non-ASCII data".to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(bytes).to_string())
}

fn decode_projected_varlen_unicode(bytes: &[u8]) -> Result<String, AppError> {
    let ptr = decode_projected_varlen_ptr(bytes)?;
    if ptr.is_null() {
        return Ok(String::new());
    }
    let bytes = unsafe { CStr::from_ptr(ptr.cast()) }.to_bytes();
    String::from_utf8(bytes.to_vec()).map_err(|e| {
        AppError::DrawingError(format!(
            "Projected UTF-8 string contained invalid data: {e}"
        ))
    })
}

fn projected_type_contains_varlen(type_desc: &TypeDescriptor) -> bool {
    match type_desc {
        TypeDescriptor::VarLenAscii
        | TypeDescriptor::VarLenUnicode
        | TypeDescriptor::VarLenArray(_) => true,
        TypeDescriptor::FixedArray(inner, _) => projected_type_contains_varlen(inner),
        _ => false,
    }
}

fn reclaim_projected_varlen_if_needed(
    field_type: &TypeDescriptor,
    out_shape: &[usize],
    bytes: &mut [u8],
) -> Result<(), AppError> {
    if !projected_type_contains_varlen(field_type) {
        return Ok(());
    }
    let dtype = Datatype::from_descriptor(field_type)?;
    let space = Dataspace::try_new(selection_mem_shape(out_shape))?;
    h5check(unsafe {
        H5Treclaim(
            dtype.id(),
            space.id(),
            H5P_DEFAULT,
            bytes.as_mut_ptr().cast(),
        )
    })
    .map(|_| ())
    .map_err(|e| AppError::DrawingError(format!("Failed to reclaim projected varlen data: {e}")))
}

pub fn root_compound_projection(
    dataset_path: &str,
    compound_type: CompoundType,
) -> CompoundFieldProjection {
    CompoundFieldProjection {
        field_path: vec![],
        field_type: TypeDescriptor::Compound(compound_type),
        virtual_path: dataset_path.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{mem::ManuallyDrop, str::FromStr};

    use hdf5_metno::types::{TypeDescriptor, VarLenUnicode};

    use super::ProjectionDecode;

    #[test]
    fn projected_scalar_varlen_unicode_strings_are_supported() {
        let value = ManuallyDrop::new(
            VarLenUnicode::from_str("hello compound").expect("failed to allocate varlen string"),
        );
        let mut bytes = (value.as_ptr() as usize).to_ne_bytes().to_vec();

        let decoded = <String as ProjectionDecode>::decode_scalar_buffer(
            &TypeDescriptor::VarLenUnicode,
            &mut bytes,
        )
        .expect("decode projected string");
        assert_eq!(decoded, "hello compound");
    }

    #[test]
    fn projected_vector_varlen_unicode_strings_are_supported() {
        let alpha =
            ManuallyDrop::new(VarLenUnicode::from_str("alpha").expect("failed to allocate alpha"));
        let beta =
            ManuallyDrop::new(VarLenUnicode::from_str("beta").expect("failed to allocate beta"));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(alpha.as_ptr() as usize).to_ne_bytes());
        bytes.extend_from_slice(&(beta.as_ptr() as usize).to_ne_bytes());

        let decoded = <String as ProjectionDecode>::decode_value_buffer(
            &TypeDescriptor::VarLenUnicode,
            &[2],
            &mut bytes,
        )
        .expect("decode projected strings");
        assert_eq!(decoded, vec!["alpha".to_string(), "beta".to_string()]);
    }
}
