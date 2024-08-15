mod api;

use std::net::Ipv4Addr;
use actix_web::{App, get, HttpResponse, HttpServer, Responder, web};
use actix_web::middleware::Logger;
use actix_web::web::{Data, Json, ServiceConfig};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use crate::api::InfuraClient;
use anyhow::{Context, Result};
use utoipa_scalar::{Scalar, Servable};

#[derive(OpenApi)]
#[openapi(
    paths(
        get_wallet_balance
    ),
    components(schemas(Wallet))
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
    request_body = Wallet,
    responses(
        (status = 200, description = "successfully got balance", body = WalletInfo),
        (status = 400, description = "bad request", body = ErrorResponse),
        (status = 500, description = "internal server error", body = ErrorResponse)
    )
)]
#[get("/address/balance")]
async fn get_wallet_balance(wallet: Json<Wallet>, infura_client: Data<InfuraClient>) -> impl Responder {
    let balance = &infura_client.get_ref().get_balance(&wallet.address).await;
    match balance {
        Ok(balance) => HttpResponse::Ok().json(WalletInfo{balance: *balance}),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse::InternalServerError(format!("{:?}", e))),
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
struct Wallet {
    address: String,
}

#[derive(Serialize)]
struct WalletInfo {
    balance: f64,
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

    Ok(HttpServer::new(move || {
        App::new().wrap(Logger::default())
            .service(web::scope("/api").configure(configure(client_data.clone())))
            .service(Scalar::with_url("/scalar", openapi.clone()))
    }).bind((Ipv4Addr::UNSPECIFIED, 8080))?.run().await?)
}
