use super::*;

fn load_request_kind_key(kind: MultiChartLoadKind) -> bool {
    matches!(kind, MultiChartLoadKind::Detail { .. })
}

pub(super) fn coalesce_load_requests(
    requests: impl IntoIterator<Item = MultiChartLoadRequest>,
) -> Vec<MultiChartLoadRequest> {
    let requests = requests.into_iter().collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let mut coalesced = Vec::new();
    for request in requests.into_iter().rev() {
        let key = (request.item_id, load_request_kind_key(request.kind));
        if seen.insert(key) {
            coalesced.push(request);
        }
    }
    coalesced.reverse();
    coalesced
}

pub fn handle_mchart_load(tx_events: Sender<AppEvent>) -> Sender<MultiChartLoadRequest> {
    let (tx_load, rx_load) = channel::<MultiChartLoadRequest>();
    thread::spawn(move || loop {
        let Ok(first_request) = rx_load.recv() else {
            return;
        };
        let mut pending_requests = vec![first_request];
        while let Ok(request) = rx_load.try_recv() {
            pending_requests.push(request);
        }

        for request in coalesce_load_requests(pending_requests) {
            let _ = tx_events.send(AppEvent::MultiChartLoad(MultiChartLoadResult::Started {
                item_id: request.item_id,
                kind: request.kind,
            }));
            let result = match (&request.kind, request.source) {
                (
                    MultiChartLoadKind::Overview { .. },
                    MultiChartLoadSource::Dataset { dataset, selection },
                ) => plot_dataset_with_cap(
                    &dataset,
                    &selection,
                    configure::current_multichart_settings().overview_max_samples,
                )
                .map(|preview| MultiChartLoadResult::Success {
                    item_id: request.item_id,
                    kind: request.kind,
                    points: preview.data,
                    source_len: preview.length,
                })
                .map_err(|error| format!("Failed loading sampled series: {error}")),
                (
                    MultiChartLoadKind::Overview { .. },
                    MultiChartLoadSource::CompoundLeaf {
                        dataset,
                        meta,
                        selection,
                    },
                ) => plot_projected_with_cap(
                    &dataset,
                    meta.as_ref(),
                    &selection,
                    configure::current_multichart_settings().overview_max_samples,
                )
                .map(|preview| MultiChartLoadResult::Success {
                    item_id: request.item_id,
                    kind: request.kind,
                    points: preview.data,
                    source_len: preview.length,
                })
                .map_err(|error| format!("Failed loading sampled series: {error}")),
                (
                    MultiChartLoadKind::Detail { window, .. },
                    MultiChartLoadSource::Dataset { dataset, selection },
                ) => {
                    let detail_selection =
                        selection_with_window(&selection, window.start, window.end);
                    plot_dataset_with_cap(&dataset, &detail_selection, window.sample_cap)
                        .map(|preview| MultiChartLoadResult::Success {
                            item_id: request.item_id,
                            kind: request.kind,
                            points: offset_points(preview.data, window.start),
                            source_len: 0,
                        })
                        .map_err(|error| format!("Failed loading viewport detail: {error}"))
                }
                (
                    MultiChartLoadKind::Detail { window, .. },
                    MultiChartLoadSource::CompoundLeaf {
                        dataset,
                        meta,
                        selection,
                    },
                ) => {
                    let detail_selection =
                        selection_with_window(&selection, window.start, window.end);
                    plot_projected_with_cap(
                        &dataset,
                        meta.as_ref(),
                        &detail_selection,
                        window.sample_cap,
                    )
                    .map(|preview| MultiChartLoadResult::Success {
                        item_id: request.item_id,
                        kind: request.kind,
                        points: offset_points(preview.data, window.start),
                        source_len: 0,
                    })
                    .map_err(|error| format!("Failed loading viewport detail: {error}"))
                }
            };
            let _ = tx_events.send(AppEvent::MultiChartLoad(match result {
                Ok(result) => result,
                Err(message) => MultiChartLoadResult::Failure {
                    item_id: request.item_id,
                    kind: request.kind,
                    message,
                },
            }));
        }
    });
    tx_load
}

pub fn handle_mchart_render(tx_events: Sender<AppEvent>) -> Sender<MultiChartRenderRequest> {
    let (tx_render, rx_render) = channel::<MultiChartRenderRequest>();
    thread::spawn(move || loop {
        let Ok(mut request) = rx_render.recv() else {
            return;
        };
        while let Ok(next_request) = rx_render.try_recv() {
            request = next_request;
        }
        let result = render::render_prepared_chart_request(request);
        let _ = tx_events.send(AppEvent::MultiChartRender(result));
    });
    tx_render
}

fn selection_with_window(
    selection: &PreviewSelection,
    start: usize,
    end: usize,
) -> PreviewSelection {
    let base_start = match selection.slice {
        SliceSelection::All => 0,
        SliceSelection::FromTo(base_start, _) => base_start,
    };
    PreviewSelection {
        index: selection.index.clone(),
        x: selection.x,
        slice: SliceSelection::FromTo(base_start + start, base_start + end),
    }
}

fn offset_points(points: Vec<Point>, start: usize) -> Vec<Point> {
    points
        .into_iter()
        .map(|(x, y)| (x + start as f64, y))
        .collect()
}
