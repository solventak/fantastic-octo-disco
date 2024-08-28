use std::net::Ipv4Addr;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web::middleware::Logger;
use prometheus::{opts, CounterVec, Encoder, histogram_opts, linear_buckets, register_counter_vec, TextEncoder, HistogramVec, register_histogram_vec};
use anyhow::Result;
use lazy_static::lazy_static;

lazy_static!{
    static ref REQUEST_LATENCY: HistogramVec = register_histogram_vec!(histogram_opts!(
        "client_http_request_latency",
        "The latency of a request in ms.",
        linear_buckets(0., 50., 10).unwrap(),
    ), &["status_code"]).unwrap();

    static ref REQUEST_COUNT: CounterVec = register_counter_vec!(opts!(
        "client_api_request_count",
        "Counter for number of requests.",
    ), &["status_code"]).unwrap();
}

async fn metrics() -> impl Responder {
    // Create an encoder for the Prometheus metrics
    let encoder = TextEncoder::new();

    // Gather the metrics
    let metric_families = prometheus::gather();

    // Encode the metrics into a buffer
    let mut buffer = vec![];
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        eprintln!("Error encoding metrics: {:?}", e);
        return HttpResponse::InternalServerError().finish();
    }

    // Return the metrics as an HTTP response
    HttpResponse::Ok()
        .content_type(encoder.format_type())
        .body(buffer)
}

async fn make_health_request() {
    let start_time = chrono::Utc::now();

    // make a request to the health endpoint
    let resp_res = reqwest::get("http://api.solventdj.com/api/health").await;

    let end_time = chrono::Utc::now();
    let duration = end_time - start_time;
    let latency = duration.num_milliseconds();
    if let Ok(response) = resp_res {
        let status_str = String::from(response.status().as_str());
        REQUEST_COUNT.with_label_values(&[status_str.as_str()]).inc();
        REQUEST_LATENCY.with_label_values(&[status_str.as_str()]).observe(latency as f64);
    }
    println!("Health check took {} ms", latency);
}

#[tokio::main]
async fn main() -> Result<()> {
    // start a thread which makes a health check request every second
    tokio::spawn(async move {
        loop {
            make_health_request().await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    Ok(HttpServer::new(move || {
        App::new().wrap(Logger::default())
            .route("/metrics", web::get().to(metrics))
    }).bind((Ipv4Addr::UNSPECIFIED, 80))?.run().await?)
}
