use hdf5_metno::Dataset;

pub trait Previewable {
    fn preview(&self, selection: PreviewSelection) -> DatasetPreview;
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
    OneDim(Slice),
}

impl Previewable for Dataset {
    fn preview(&self, selection: PreviewSelection) -> DatasetPreview {
        let data_to_show = match selection {
            PreviewSelection::OneDim(slice) => match slice {
                Slice::All => self.read_1d::<f64>().unwrap(),
            },
        };

        let data = data_to_show
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
