use super::*;
use crate::ui::perf;

fn load_request_kind_key(kind: MultiChartLoadKind) -> bool {
    matches!(kind, MultiChartLoadKind::Detail { .. })
}

fn load_request_priority(kind: MultiChartLoadKind) -> u8 {
    match kind {
        MultiChartLoadKind::Detail { .. } => 0,
        MultiChartLoadKind::Overview { .. } => 1,
    }
}

pub(super) fn coalesce_load_requests(
    requests: impl IntoIterator<Item = MultiChartLoadRequest>,
) -> Vec<MultiChartLoadRequest> {
    let requests = requests.into_iter().enumerate().collect::<Vec<_>>();
    let input_len = requests.len() as u64;
    let mut latest_requests = HashMap::new();
    for (index, request) in requests {
        let key = (request.item_id, load_request_kind_key(request.kind));
        latest_requests.insert(key, (index, request));
    }
    let mut coalesced = latest_requests.into_values().collect::<Vec<_>>();
    coalesced.sort_by(|(left_index, left_request), (right_index, right_request)| {
        load_request_priority(left_request.kind)
            .cmp(&load_request_priority(right_request.kind))
            .then_with(|| right_index.cmp(left_index))
            .then_with(|| left_request.item_id.cmp(&right_request.item_id))
    });
    let coalesced = coalesced
        .into_iter()
        .map(|(_, request)| request)
        .collect::<Vec<_>>();
    perf::metrics().mchart.load_batches.increment();
    perf::metrics().mchart.load_requests_seen.add(input_len);
    let coalesced_count = input_len.saturating_sub(coalesced.len() as u64);
    if coalesced_count > 0 {
        perf::metrics()
            .mchart
            .load_requests_coalesced
            .add(coalesced_count);
    }
    coalesced
}

pub fn handle_mchart_load(tx_events: Sender<AppEvent>) -> Sender<MultiChartLoadRequest> {
    let (tx_load, rx_load) = channel::<MultiChartLoadRequest>();
    thread::spawn(move || {
        let mut pending_requests = Vec::new();
        loop {
            if pending_requests.is_empty() {
                let Ok(first_request) = rx_load.recv() else {
                    return;
                };
                pending_requests.push(first_request);
            }
            while let Ok(request) = rx_load.try_recv() {
                pending_requests.push(request);
            }
            let mut coalesced_requests =
                coalesce_load_requests(std::mem::take(&mut pending_requests));
            let Some(request) = coalesced_requests.first().cloned() else {
                continue;
            };
            coalesced_requests.remove(0);
            pending_requests = coalesced_requests;

            let _load_timer = perf::metrics().mchart.load_worker_total.start();
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
        let mut drained_requests = 0_u64;
        while let Ok(next_request) = rx_render.try_recv() {
            request = next_request;
            drained_requests += 1;
        }
        if drained_requests > 0 {
            perf::metrics()
                .mchart
                .render_requests_drained
                .add(drained_requests);
        }
        let _render_timer = perf::metrics().mchart.render_worker_total.start();
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
