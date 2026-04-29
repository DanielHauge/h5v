use hdf5_metno::{
    types::{CompoundType, FloatSize, IntSize, TypeDescriptor},
    Dataset, Dataspace, Selection,
};
use ndarray::{Array1, Array2};

use crate::{
    data::{DatasetPlotingData, PreviewSelection, SliceSelection},
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
}

const H5_DEFAULT_ID: i64 = 0;

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

fn read_projected_bytes(
    dataset: &Dataset,
    projection: &CompoundFieldProjection,
    selection: Selection,
) -> Result<(Vec<u8>, Vec<usize>), AppError> {
    let dtype = dataset.dtype()?;
    let item_size = dtype.size();
    let out_shape = selection_to_shape(&selection, dataset)?;
    let total_elems = out_shape.iter().product::<usize>();

    let file_space = dataset.space()?.copy();
    let raw_selection = selection.into_raw(dataset.shape())?;
    unsafe {
        raw_selection.apply_to_dataspace(file_space.id())?;
    }
    let mem_space = Dataspace::try_new(out_shape.clone())?;

    let mut buffer = vec![0_u8; total_elems * item_size];
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
    let mut out = Vec::with_capacity(total_elems * field_size);
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

pub trait ProjectionDecode: Sized {
    fn decode(field_type: &TypeDescriptor, bytes: &[u8]) -> Result<Self, AppError>;
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
            TypeDescriptor::FixedAscii(_) | TypeDescriptor::FixedUnicode(_) => {
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
            }
            TypeDescriptor::VarLenAscii | TypeDescriptor::VarLenUnicode => {
                Err(AppError::DrawingError(
                    "Varlen strings inside compound fields are not supported yet".to_string(),
                ))
            }
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
    let bytes = read_projected_scalar_bytes(dataset, projection)?;
    T::decode(&projection.field_type, &bytes)
}

pub fn read_projected_values_1d<T: ProjectionDecode>(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: Selection,
) -> Result<Array1<T>, AppError> {
    let projection = meta.compound_projection.as_ref().ok_or_else(|| {
        AppError::DrawingError("Projected values requested for non-compound dataset".to_string())
    })?;
    let field_size = projection.field_type.size();
    let (bytes, out_shape) = read_projected_bytes(dataset, projection, selection)?;
    if out_shape.len() != 1 {
        return Err(AppError::DrawingError(format!(
            "Expected 1D projected shape, got {:?}",
            out_shape
        )));
    }
    let values = bytes
        .chunks_exact(field_size)
        .map(|chunk| T::decode(&projection.field_type, chunk))
        .collect::<Result<Vec<_>, _>>()?;
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
    let field_size = projection.field_type.size();
    let (bytes, out_shape) = read_projected_bytes(dataset, projection, selection)?;
    if out_shape.len() != 2 {
        return Err(AppError::DrawingError(format!(
            "Expected 2D projected shape, got {:?}",
            out_shape
        )));
    }
    let values = bytes
        .chunks_exact(field_size)
        .map(|chunk| T::decode(&projection.field_type, chunk))
        .collect::<Result<Vec<_>, _>>()?;
    Array2::from_shape_vec((out_shape[0], out_shape[1]), values).map_err(|e| {
        AppError::DrawingError(format!("Failed shaping projected 2D field values: {e}"))
    })
}

pub fn plot_projected(
    dataset: &Dataset,
    meta: &DatasetMeta,
    selection: &PreviewSelection,
) -> Result<DatasetPlotingData, AppError> {
    let slice = match selection.slice {
        SliceSelection::All => 0..dataset.shape()[selection.x],
        SliceSelection::FromTo(a, b) => a..b,
    };
    let mut slice_selections = Vec::new();
    for idx in 0..dataset.shape().len() {
        if idx == selection.x {
            slice_selections.push(hdf5_metno::SliceOrIndex::SliceTo {
                start: slice.start,
                step: 1,
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
        .map(|(idx, value)| (idx as f64, *value))
        .collect::<Vec<_>>();
    let length = data.len();
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
