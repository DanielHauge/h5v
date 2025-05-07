use hdf5_metno::{Dataset, Error, H5Type, Selection};
use ndarray::{s, Array1, Array2};

pub trait Plotable {
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

pub struct DatasetPlotingData {
    pub data: Vec<(f64, f64)>,
    pub length: usize,
    pub max: f64,
    pub min: f64,
}

pub struct DatasetTableData<T> {
    pub data: Array2<T>,
}

pub struct DatasetValuesData<T> {
    pub data: Array1<T>,
}

pub enum SliceSelection {
    All,
    // FromTo(usize, usize),
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

impl Plotable for Dataset {
    fn plot(&self, selection: PreviewSelection) -> Result<DatasetPlotingData, Error> {
        let slice = match selection.slice {
            // SliceSelection::FromTo(start, end) => start..end,
            SliceSelection::All => 0..self.shape()[selection.x],
        };
        // TODO: Fix this, use the way to make slice from selections, like dims selector.
        let data_to_show = match selection.index.len() {
            0 => self.read_1d::<f64>()?,
            1 => match selection.x {
                0 => self.read_slice(s![slice, selection.index[0]])?,
                _ => self.read_slice(s![selection.index[0], slice])?,
            },
            2 => match selection.x {
                0 => self.read_slice(s![slice, selection.index[0], selection.index[1]])?,
                1 => self.read_slice(s![selection.index[0], slice, selection.index[1]])?,
                _ => self.read_slice(s![selection.index[0], selection.index[1], slice])?,
            },
            3 => match selection.x {
                0 => self.read_slice(s![
                    slice,
                    selection.index[0],
                    selection.index[1],
                    selection.index[2]
                ])?,
                1 => self.read_slice(s![
                    selection.index[0],
                    slice,
                    selection.index[1],
                    selection.index[2]
                ])?,
                2 => self.read_slice(s![
                    selection.index[0],
                    selection.index[1],
                    slice,
                    selection.index[2]
                ])?,
                _ => self.read_slice(s![
                    selection.index[0],
                    selection.index[1],
                    selection.index[2],
                    slice
                ])?,
            },
            4 => match selection.x {
                0 => self.read_slice(s![
                    slice,
                    selection.index[0],
                    selection.index[1],
                    selection.index[2],
                    selection.index[3]
                ])?,
                1 => self.read_slice(s![
                    selection.index[0],
                    slice,
                    selection.index[1],
                    selection.index[2],
                    selection.index[3]
                ])?,
                2 => self.read_slice(s![
                    selection.index[0],
                    selection.index[1],
                    slice,
                    selection.index[2],
                    selection.index[3]
                ])?,
                3 => self.read_slice(s![
                    selection.index[0],
                    selection.index[1],
                    selection.index[2],
                    slice,
                    selection.index[3]
                ])?,
                _ => self.read_slice(s![
                    selection.index[0],
                    selection.index[1],
                    selection.index[2],
                    selection.index[3],
                    slice
                ])?,
            },
            _ => {
                return Err(Error::from("Cmon man, who can think in +5D? xD"));
            }
        };

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
