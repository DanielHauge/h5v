use hdf5_metno::{Dataset, Error, H5Type, Hyperslab, Selection, SliceOrIndex};
use ndarray::{Array1, Array2};

pub trait Previewable {
    fn plot(&self, selection: PreviewSelection) -> Result<DatasetPlotingData, Error>;
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
        let mut lengths = Vec::with_capacity(self.data.shape()[0]);
        (0..self.data.shape()[0]).for_each(|i| {
            lengths.push(0);
            for j in 0..self.data.shape()[1] - 1 {
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

pub enum SliceSelection {
    All,
    FromTo(usize, usize),
}
type XAxis = usize;

pub struct PreviewSelection {
    pub index: Vec<usize>,
    pub x: XAxis,
    pub slice: SliceSelection,
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
    fn plot(&self, selection: PreviewSelection) -> Result<DatasetPlotingData, Error> {
        let slice = match selection.slice {
            SliceSelection::All => 0..self.shape()[selection.x],
            SliceSelection::FromTo(a, b) => a..b,
        };

        let mut slice_selections: Vec<SliceOrIndex> = Vec::new();
        for idx in 0..self.shape().len() {
            if idx == selection.x {
                slice_selections.push(SliceOrIndex::SliceTo {
                    start: slice.start,
                    step: 1,
                    end: slice.end,
                    block: 1,
                });
            } else {
                slice_selections.push(SliceOrIndex::Index(selection.index[idx]));
            }
        }

        let selection = Selection::Hyperslab(Hyperslab::from(slice_selections));
        let data_to_show = self.read_slice_1d(selection)?;

        let data = data_to_show
            .iter()
            .enumerate()
            .map(|(i, y)| (i as f64, *y))
            .collect::<Vec<_>>();
        let length = data.len();
        let max = data.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        let min = data.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        Ok(DatasetPlotingData {
            data,
            length,
            max,
            min,
        })
    }
}
