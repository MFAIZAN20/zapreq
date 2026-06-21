use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::SourceSelector;
use crate::config::Config;
use crate::localdb::{open_connection, record_report};
use crate::sources::{execute_record, resolve_records, RequestRecord};

#[derive(Clone, Debug, Serialize)]
pub struct EndpointBenchmark {
    pub endpoint: String,
    pub concurrency: u32,
    pub samples: usize,
    pub success_count: usize,
    pub error_count: usize,
    pub min_ms: u64,
    pub avg_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub max_ms: u64,
    pub avg_size_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkReport {
    pub source: String,
    pub generated_at: String,
    pub iterations: u32,
    pub duration_secs: Option<u64>,
    pub concurrency: u32,
    pub endpoints: Vec<EndpointBenchmark>,
    pub report_id: i64,
}

pub fn benchmark(
    selector: &SourceSelector,
    iterations: u32,
    duration_secs: Option<u64>,
    concurrency: u32,
    config: &Config,
) -> Result<BenchmarkReport> {
    let records = resolve_records(selector)?;
    let source = source_name(selector);
    let iterations = iterations.max(1);
    let concurrency = concurrency.max(1);
    let mut endpoints = Vec::new();
    for record in records {
        endpoints.push(run_endpoint_benchmark(
            &record,
            iterations,
            duration_secs,
            concurrency,
            config,
        ));
    }

    let report = BenchmarkReport {
        source: source.clone(),
        generated_at: Utc::now().to_rfc3339(),
        iterations,
        duration_secs,
        concurrency,
        endpoints,
        report_id: 0,
    };
    let summary = format!(
        "Benchmarked {} endpoint(s) from {} with concurrency {}",
        report.endpoints.len(),
        source,
        concurrency
    );
    let conn = open_connection()?;
    let report_id = record_report(
        &conn,
        "performance",
        &source,
        &summary,
        &serde_json::to_string_pretty(&report)?,
    )?;
    Ok(BenchmarkReport {
        report_id,
        ..report
    })
}

pub fn render_report(report: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Performance benchmark for {} [{}] report_id={} concurrency={}\n",
        report.source, report.generated_at, report.report_id, report.concurrency
    ));
    for endpoint in &report.endpoints {
        out.push_str(&format!(
            "- {} :: concurrency={} samples={} success={} errors={} min={}ms avg={}ms p50={}ms p95={}ms p99={}ms max={}ms avg_size={}B\n",
            endpoint.endpoint,
            endpoint.concurrency,
            endpoint.samples,
            endpoint.success_count,
            endpoint.error_count,
            endpoint.min_ms,
            endpoint.avg_ms,
            endpoint.p50_ms,
            endpoint.p95_ms,
            endpoint.p99_ms,
            endpoint.max_ms,
            endpoint.avg_size_bytes
        ));
    }
    out
}

fn run_endpoint_benchmark(
    record: &RequestRecord,
    iterations: u32,
    duration_secs: Option<u64>,
    concurrency: u32,
    config: &Config,
) -> EndpointBenchmark {
    let iterations = iterations.max(1);
    let concurrency = concurrency.max(1);
    let started = Instant::now();

    if concurrency == 1 {
        return summarize_endpoint(
            record.source_label.clone(),
            concurrency,
            vec![run_benchmark_worker(
                record.clone(),
                iterations,
                duration_secs,
                config.clone(),
                started,
            )],
        );
    }

    let mut handles = Vec::with_capacity(concurrency as usize);
    for _ in 0..concurrency {
        let record = record.clone();
        let config = config.clone();
        handles.push(thread::spawn(move || {
            run_benchmark_worker(record, iterations, duration_secs, config, started)
        }));
    }

    let worker_runs = handles
        .into_iter()
        .map(|handle| handle.join().expect("perf worker thread panicked"))
        .collect::<Vec<_>>();

    summarize_endpoint(record.source_label.clone(), concurrency, worker_runs)
}

#[derive(Default)]
struct WorkerRun {
    latencies: Vec<u64>,
    sizes: Vec<usize>,
    success_count: usize,
    error_count: usize,
}

fn run_benchmark_worker(
    record: RequestRecord,
    iterations: u32,
    duration_secs: Option<u64>,
    config: Config,
    started: Instant,
) -> WorkerRun {
    let mut run = WorkerRun::default();
    let max_duration = duration_secs.map(Duration::from_secs);
    let mut runs = 0u32;

    loop {
        if let Some(limit) = max_duration {
            if runs >= 1 && started.elapsed() >= limit {
                break;
            }
        } else if runs >= iterations {
            break;
        }

        match execute_record(&record, &config) {
            Ok((_trace, response, elapsed_ms)) => {
                run.latencies.push(elapsed_ms);
                run.sizes.push(response.body.len());
                run.success_count += 1;
            }
            Err(_) => {
                run.error_count += 1;
            }
        }
        runs += 1;
    }

    run
}

fn summarize_endpoint(
    endpoint: String,
    concurrency: u32,
    worker_runs: Vec<WorkerRun>,
) -> EndpointBenchmark {
    let mut latencies = Vec::new();
    let mut sizes = Vec::new();
    let mut success_count = 0usize;
    let mut error_count = 0usize;

    for mut worker_run in worker_runs {
        latencies.append(&mut worker_run.latencies);
        sizes.append(&mut worker_run.sizes);
        success_count += worker_run.success_count;
        error_count += worker_run.error_count;
    }

    latencies.sort_unstable();
    let samples = latencies.len();
    let avg_ms = if samples == 0 {
        0
    } else {
        latencies.iter().sum::<u64>() / samples as u64
    };
    let avg_size_bytes = if sizes.is_empty() {
        0
    } else {
        sizes.iter().sum::<usize>() / sizes.len()
    };

    EndpointBenchmark {
        endpoint,
        concurrency,
        samples,
        success_count,
        error_count,
        min_ms: latencies.first().copied().unwrap_or(0),
        avg_ms,
        p50_ms: percentile(&latencies, 0.50),
        p95_ms: percentile(&latencies, 0.95),
        p99_ms: percentile(&latencies, 0.99),
        max_ms: latencies.last().copied().unwrap_or(0),
        avg_size_bytes,
    }
}

fn percentile(values: &[u64], fraction: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let idx = ((values.len() - 1) as f64 * fraction).round() as usize;
    values[idx.min(values.len() - 1)]
}

fn source_name(selector: &SourceSelector) -> String {
    if let Some(alias) = selector.alias.as_deref() {
        return format!("alias:{alias}");
    }
    if let Some(workspace) = selector.workspace.as_deref() {
        if let Some(request) = selector.request.as_deref() {
            return format!("request:{workspace}/{request}");
        }
        return format!("workspace:{workspace}");
    }
    selector
        .file
        .as_deref()
        .map(|path| format!("file:{path}"))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::{percentile, summarize_endpoint, WorkerRun};

    #[test]
    fn percentile_handles_sorted_values() {
        let values = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&values, 0.50), 30);
        assert_eq!(percentile(&values, 0.95), 50);
    }

    #[test]
    fn summarize_endpoint_aggregates_worker_samples() {
        let report = summarize_endpoint(
            "alias:demo".to_string(),
            2,
            vec![
                WorkerRun {
                    latencies: vec![10, 30],
                    sizes: vec![100, 300],
                    success_count: 2,
                    error_count: 1,
                },
                WorkerRun {
                    latencies: vec![20, 40, 50],
                    sizes: vec![200, 400, 500],
                    success_count: 3,
                    error_count: 2,
                },
            ],
        );

        assert_eq!(report.endpoint, "alias:demo");
        assert_eq!(report.concurrency, 2);
        assert_eq!(report.samples, 5);
        assert_eq!(report.success_count, 5);
        assert_eq!(report.error_count, 3);
        assert_eq!(report.min_ms, 10);
        assert_eq!(report.avg_ms, 30);
        assert_eq!(report.p50_ms, 30);
        assert_eq!(report.p95_ms, 50);
        assert_eq!(report.p99_ms, 50);
        assert_eq!(report.max_ms, 50);
        assert_eq!(report.avg_size_bytes, 300);
    }
}
