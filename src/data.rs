use core::f64;

use hdf5_metno::{Dataset, Error, H5Type, Hyperslab, Selection, SliceOrIndex};
use ndarray::{Array1, Array2};

pub(crate) const MAX_PLOT_SAMPLES: usize = 4096;

pub trait Previewable {
    fn plot(&self, selection: &PreviewSelection) -> Result<DatasetPlotingData, Error>;
}

pub trait MatrixTable {
    fn matrix_table<T>(&self, selection: Selection) -> Result<DatasetTableData<T>, Error>
    where
        T: H5Type;
}

pub trait MatrixValues {
    fn matrix_values<T>(&self, selection: Selection) -> Result<DatasetValuesData<T>, Error>
    where
        T: H5Type;
}

pub trait StringLengths {
    #[allow(dead_code)]
    fn string_lengths(&self) -> Vec<usize>;
}

#[derive(Debug, Clone)]
pub struct DatasetPlotingData {
    pub data: Vec<(f64, f64)>,
    pub length: usize,
    pub max: f64,
    pub min: f64,
}

pub struct DatasetTableData<T> {
    pub data: Array2<T>,
}

impl StringLengths for DatasetTableData<String> {
    fn string_lengths(&self) -> Vec<usize> {
        let shape = self.data.shape();
        if shape.len() < 2 {
            return Vec::new();
        }
        let rows = shape[0];
        let cols = shape[1];
        let mut lengths = Vec::with_capacity(rows);
        (0..rows).for_each(|i| {
            lengths.push(0);
            for j in 0..cols.saturating_sub(1) {
                let len = self.data[[i, j]].len();
                if lengths[i] + 2 < len {
                    lengths[i] = len + 2;
                }
            }
        });
        lengths
    }
}

impl From<DatasetTableData<f64>> for DatasetTableData<String> {
    fn from(val: DatasetTableData<f64>) -> Self {
        let data = val.data.mapv(|x| format!("{}", x));
        DatasetTableData { data }
    }
}

pub struct DatasetValuesData<T> {
    pub data: Array1<T>,
}

#[derive(Debug, Clone)]
pub enum SliceSelection {
    All,
    FromTo(usize, usize),
}

impl PartialEq for SliceSelection {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SliceSelection::All, SliceSelection::All) => true,
            (SliceSelection::FromTo(a1, b1), SliceSelection::FromTo(a2, b2)) => {
                a1 == a2 && b1 == b2
            }
            _ => false,
        }
    }
}

type XAxis = usize;

#[derive(Debug, Clone)]
pub struct PreviewSelection {
    pub index: Vec<usize>,
    pub x: XAxis,
    pub slice: SliceSelection,
}

pub(crate) fn validate_preview_selection_shape(
    shape: &[usize],
    selection: &PreviewSelection,
) -> Result<(), Error> {
    if selection.x >= shape.len() {
        return Err(Error::from(format!(
            "Preview selection x-axis {} is out of bounds for shape {:?}",
            selection.x, shape
        )));
    }
    if selection.index.len() < shape.len() {
        return Err(Error::from(format!(
            "Preview selection index rank {} does not match shape rank {}",
            selection.index.len(),
            shape.len()
        )));
    }
    for (idx, dim_len) in shape.iter().copied().enumerate() {
        if idx == selection.x {
            continue;
        }
        if selection.index[idx] >= dim_len {
            return Err(Error::from(format!(
                "Preview selection index {} is out of bounds for dim {} with length {}",
                selection.index[idx], idx, dim_len
            )));
        }
    }
    match selection.slice {
        SliceSelection::All => {}
        SliceSelection::FromTo(start, end) => {
            let axis_len = shape[selection.x];
            if start > end || end > axis_len {
                return Err(Error::from(format!(
                    "Preview selection slice {}..{} is invalid for axis length {}",
                    start, end, axis_len
                )));
            }
        }
    }
    Ok(())
}

impl PartialEq for PreviewSelection {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.x == other.x && self.slice == other.slice
    }
}

impl MatrixTable for Dataset {
    fn matrix_table<T>(&self, selection: Selection) -> Result<DatasetTableData<T>, Error>
    where
        T: H5Type,
    {
        let gg = self.read_slice_2d(selection)?;
        let result = DatasetTableData { data: gg };
        Ok(result)
    }
}

impl MatrixValues for Dataset {
    fn matrix_values<T>(&self, selection: Selection) -> Result<DatasetValuesData<T>, Error>
    where
        T: H5Type,
    {
        let data = self.read_slice_1d(selection)?;
        let result = DatasetValuesData { data };
        Ok(result)
    }
}

impl Previewable for Dataset {
    fn plot(&self, selection: &PreviewSelection) -> Result<DatasetPlotingData, Error> {
        plot_dataset_with_cap(self, selection, MAX_PLOT_SAMPLES)
    }
}

pub(crate) fn plot_dataset_with_cap(
    dataset: &Dataset,
    selection: &PreviewSelection,
    max_samples: usize,
) -> Result<DatasetPlotingData, Error> {
    let shape = dataset.shape();
    validate_preview_selection_shape(&shape, selection)?;
    let slice = match selection.slice {
        SliceSelection::All => 0..shape[selection.x],
        SliceSelection::FromTo(a, b) => a..b,
    };
    let length = slice.end.saturating_sub(slice.start);
    let step = plot_sampling_step_with_cap(length, max_samples);

    let mut slice_selections: Vec<SliceOrIndex> = Vec::new();
    for idx in 0..shape.len() {
        if idx == selection.x {
            slice_selections.push(SliceOrIndex::SliceTo {
                start: slice.start,
                step,
                end: slice.end,
                block: 1,
            });
        } else {
            slice_selections.push(SliceOrIndex::Index(selection.index[idx]));
        }
    }

    let selection = Selection::Hyperslab(Hyperslab::from(slice_selections));
    let data_to_show = dataset.read_slice_1d(selection)?;
    let data = data_to_show
        .iter()
        .enumerate()
        .map(|(i, y)| ((i * step) as f64, *y))
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

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn plot_sampling_step(length: usize) -> usize {
    plot_sampling_step_with_cap(length, MAX_PLOT_SAMPLES)
}

pub(crate) fn plot_sampling_step_with_cap(length: usize, max_samples: usize) -> usize {
    length.div_ceil(max_samples.max(1)).max(1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        plot_sampling_step, validate_preview_selection_shape, DatasetTableData, PreviewSelection,
        SliceSelection, StringLengths,
    };
    use ndarray::Array2;

    #[test]
    fn preview_selection_validation_rejects_short_index_rank() {
        let selection = PreviewSelection {
            index: vec![0],
            x: 1,
            slice: SliceSelection::All,
        };
        let error = validate_preview_selection_shape(&[3, 4], &selection).unwrap_err();
        assert!(error.to_string().contains("rank"));
    }

    #[test]
    fn preview_selection_validation_rejects_out_of_bounds_slice() {
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::FromTo(0, 9),
        };
        let error = validate_preview_selection_shape(&[3, 4], &selection).unwrap_err();
        assert!(error.to_string().contains("invalid"));
    }

    #[test]
    fn string_lengths_handles_empty_second_dimension() {
        let data = DatasetTableData {
            data: Array2::<String>::default((2, 0)),
        };
        assert_eq!(data.string_lengths(), vec![0, 0]);
    }

    #[test]
    fn plot_sampling_step_caps_large_previews() {
        assert_eq!(plot_sampling_step(16), 1);
        assert_eq!(plot_sampling_step(4096), 1);
        assert_eq!(plot_sampling_step(4097), 2);
        assert_eq!(plot_sampling_step(10_000), 3);
    }
}
