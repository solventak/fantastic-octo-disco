mod api;

use std::net::Ipv4Addr;
use std::time::Duration;
use actix_web::{App, get, HttpResponse, HttpServer, Responder, web};
use actix_web::middleware::Logger;
use actix_web::web::{Data, Path, ServiceConfig};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use crate::api::InfuraClient;
use anyhow::{Context, Result};
use backon::{ExponentialBuilder, Retryable};
use lazy_static::lazy_static;
use prometheus::{Encoder, Histogram, histogram_opts, linear_buckets, register_histogram, register_counter, TextEncoder, opts, register_counter_vec, labels, CounterVec};
use tokio::sync::Mutex;
use utoipa_scalar::{Scalar, Servable};

lazy_static! {
    static ref REQUEST_LATENCY: Histogram = register_histogram!(histogram_opts!(
        "http_request_latency",
        "The latency of a request in us.",
        linear_buckets(0., 100., 10).unwrap(),
    )).unwrap();

    static ref REQUEST_COUNT: CounterVec = register_counter_vec!(opts!(
        "http_request_count",
        "The number of requests.",
    ),&["request_code"]).unwrap();
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_wallet_balance
    )
)]
struct BalanceApi;

fn configure(infura_client: Data<Mutex<InfuraClient>>) -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .app_data(infura_client)
            .service(get_wallet_balance)
            .service(health);
    }
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
enum ErrorResponse {
    InternalServerError(String),
    BadRequest(String),
}

#[utoipa::path(
    responses(
        (status = 200, description = "healthy"),
    )
)]
#[get("/health")]
async fn health() -> impl Responder {
    REQUEST_COUNT.with_label_values(&["200"]).inc();
    HttpResponse::Ok().body("healthy")
}

#[utoipa::path(
    request_body = Wallet,
    responses(
        (status = 200, description = "successfully got balance", body = WalletInfo),
        (status = 400, description = "bad request", body = ErrorResponse),
        (status = 500, description = "internal server error", body = ErrorResponse)
    )
)]
#[get("/address/balance/{address}")]
async fn get_wallet_balance(address: Path<String>, infura_client: Data<Mutex<InfuraClient>>) -> impl Responder {
    let start = std::time::Instant::now();

    // add retry
    let api_call = || async {
        infura_client.lock().await.get_balance(&address).await
    };
    let retry_strategy = ExponentialBuilder::default().with_factor(2.).with_min_delay(Duration::from_millis(100)).with_max_delay(Duration::from_secs(500)).with_max_times(4);
    let balance = api_call.retry(&retry_strategy).await;
    let ret = match balance {
        Ok(balance) => HttpResponse::Ok().json(WalletInfo{balance}),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse::InternalServerError(format!("{:?}", e))),
    };
    let end = std::time::Instant::now();
    REQUEST_COUNT.with_label_values(&[ret.status().as_str()]).inc();
    REQUEST_LATENCY.observe(end.duration_since(start).as_micros() as f64);
    ret
}

#[utoipa::path(
    request_body = Wallet,
)]
#[get("/transaction/{transaction_hash}")]
async fn get_transaction(transaction_hash: Path<String>, infura_client: Data<Mutex<InfuraClient>>) -> impl Responder {
    return HttpResponse::Ok().json(infura_client.lock().await.get_transaction(&transaction_hash).await.unwrap());
}

#[derive(Serialize)]
struct WalletInfo {
    balance: f64,
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

#[actix_web::main]
async fn main() -> Result<()> {
    let infura_client = InfuraClient::new().with_context(|| "couldn't get a connection to the infura API")?;
    let client_data = Data::new(Mutex::new(infura_client));

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api", api = BalanceApi)
        ),
        tags(
            (name = "Wallet", description = "Wallet Info Endpoints"),
        ),
    )]
    struct ApiDoc;

    let openapi = ApiDoc::openapi();
    // testing cache action
    Ok(HttpServer::new(move || {
        App::new().wrap(Logger::default())
            .service(web::scope("/api").configure(configure(client_data.clone())))
            .service(Scalar::with_url("/scalar", openapi.clone()))
            .route("/metrics", web::get().to(metrics))
    }).bind((Ipv4Addr::UNSPECIFIED, 8080))?.run().await?)
}
