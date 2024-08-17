mod api;

use std::net::Ipv4Addr;
use actix_web::{App, get, HttpResponse, HttpServer, Responder, web};
use actix_web::middleware::Logger;
use actix_web::web::{Data, Json, Path, ServiceConfig};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use crate::api::InfuraClient;
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use prometheus::{Encoder, Gauge, Histogram, histogram_opts, linear_buckets, register_histogram, TextEncoder};
use utoipa_scalar::{Scalar, Servable};

lazy_static! {
    static ref REQUEST_LATENCY: Histogram = register_histogram!(histogram_opts!(
        "http_request_latency",
        "The latency of a request in ms.",
        linear_buckets(0., 5., 100).unwrap(),
    )).unwrap();
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_wallet_balance
    )
)]
struct BalanceApi;

fn configure(infura_client: Data<InfuraClient>) -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config
            .app_data(infura_client)
            .service(get_wallet_balance);
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
async fn get_wallet_balance(address: Path<String>, infura_client: Data<InfuraClient>) -> impl Responder {
    let start = std::time::Instant::now();
    let balance = &infura_client.get_ref().get_balance(&address).await;
    let ret = match balance {
        Ok(balance) => HttpResponse::Ok().json(WalletInfo{balance: *balance}),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse::InternalServerError(format!("{:?}", e))),
    };
    let end = std::time::Instant::now();
    REQUEST_LATENCY.observe(end.duration_since(start).as_millis() as f64);
    ret
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
    let client_data = Data::new(infura_client);

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
