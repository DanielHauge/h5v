use hdf5_metno::Dataset;

pub trait Previewable {
    fn preview(&self) -> DatasetPreview;
}

pub struct DatasetPreview {
    pub data: Vec<(f64, f64)>,
    pub length: usize,
    pub max: f64,
    pub min: f64,
}

pub enum Slice {
    All,
}

pub enum PreviewSelection {
    SingleDim(Slice),
}

impl Previewable for Dataset {
    fn preview(&self) -> DatasetPreview {
        let gg = self.read_1d::<f64>().unwrap();

        let data = gg
            .iter()
            .enumerate()
            .map(|(i, y)| (i as f64, *y))
            .collect::<Vec<_>>();
        let length = data.len();
        let max = data.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        let min = data.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        DatasetPreview {
            data,
            length,
            max,
            min,
        }
    }
}
