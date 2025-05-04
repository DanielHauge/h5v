use hdf5_metno::{Dataset, Error};
use ndarray::s;

pub trait Previewable {
    fn preview(&self, selection: PreviewSelection) -> Result<DatasetPreview, Error>;
}

pub struct DatasetPreview {
    pub data: Vec<(f64, f64)>,
    pub length: usize,
    pub max: f64,
    pub min: f64,
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

impl Previewable for Dataset {
    fn preview(&self, selection: PreviewSelection) -> Result<DatasetPreview, Error> {
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
        Ok(DatasetPreview {
            data,
            length,
            max,
            min,
        })
    }
}
