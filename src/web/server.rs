use crate::folding::FoldingEngine;
use crate::metrics;
use tiny_http::{Header, Response, Server};

pub fn start_server(addr: &str) {
    let server = Server::http(addr).expect("Failed to start HTTP server");
    std::println!("Web dashboard server running on http://{}", addr);

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url();
            match url {
                "/" | "/index.html" => {
                    let html = include_str!("../../html/index.html");
                    let response = Response::from_string(html).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    );
                    let _ = request.respond(response);
                }
                "/api/metrics" => {
                    let snr =
                        metrics::METRIC_CURRENT_SNR.load(std::sync::atomic::Ordering::Relaxed);
                    let uptime =
                        metrics::METRIC_UPTIME_TICKS.load(std::sync::atomic::Ordering::Relaxed);
                    let blocks_ingested =
                        metrics::METRIC_BLOCKS_INGESTED.load(std::sync::atomic::Ordering::Relaxed);
                    let blocks_dropped =
                        metrics::METRIC_BLOCKS_DROPPED.load(std::sync::atomic::Ordering::Relaxed);
                    let fft_cycles =
                        metrics::METRIC_FFT_CYCLES.load(std::sync::atomic::Ordering::Relaxed);
                    let fold_count =
                        metrics::METRIC_FOLD_COUNT.load(std::sync::atomic::Ordering::Relaxed);

                    let detected = snr >= crate::folding::SNR_THRESHOLD;

                    let json = format!(
                        "{{\"uptime_ticks\":{},\"blocks_ingested\":{},\"blocks_dropped\":{},\"fft_cycles\":{},\"fold_count\":{},\"profile_snr\":{},\"detected\":{}}}",
                        uptime, blocks_ingested, blocks_dropped, fft_cycles, fold_count, snr, detected
                    );

                    let response = Response::from_string(json)
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );
                    let _ = request.respond(response);
                }
                "/api/profile" => {
                    let folding = FoldingEngine::new();
                    let mut bins_json = String::new();
                    bins_json.push('[');
                    for i in 0..crate::folding::N_BINS {
                        let val = folding.get_bin(i);
                        bins_json.push_str(&val.to_string());
                        if i < crate::folding::N_BINS - 1 {
                            bins_json.push(',');
                        }
                    }
                    bins_json.push(']');

                    let response = Response::from_string(bins_json)
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );
                    let _ = request.respond(response);
                }
                _ => {
                    let response = Response::from_string("Not Found").with_status_code(404);
                    let _ = request.respond(response);
                }
            }
        }
    });
}
