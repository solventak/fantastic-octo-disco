use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{Counter, opts, register_counter};
use redis::Commands;
use serde::{Deserialize, Serialize};

static INFURA_ADDR: &str = "https://mainnet.infura.io/v3";
static CACHE_TTL: usize = 10;

lazy_static! {
    static ref CACHE_HIT_COUNT_METRIC: Counter = register_counter!(opts!(
        "cache_hit_count",
        "The number of cache hits.",
    )).unwrap();

    static ref API_ENDPOINT_REQUEST_COUNT_METRIC: Counter = register_counter!(opts!(
        "api_endpoint_request_count",
        "Number of times the Infura API was requested.",
    )).unwrap();
}

#[async_trait]
trait Cache<K, V>: Send + Sync {
    async fn read(&mut self, key: K) -> Result<Option<V>>;
    async fn write(&mut self, key: K, val: V) -> Result<()>;
}

struct RedisCache {
    client: redis::Client,
    conn: redis::Connection,
}

impl RedisCache {
    pub fn new(conn_addr: &str) -> Result<Self> {
        let client = redis::Client::open(conn_addr)?;
        let connection = client.get_connection()?;
        Ok(RedisCache { client, conn: connection })
    }
}

#[async_trait]
impl Cache<String, f64> for RedisCache {
    async fn read(&mut self, key: String) -> Result<Option<f64>> {
        self.conn.get(key).map_err(|e| anyhow!("failed to read from cache: {}", e))
    }

    async fn write(&mut self, key: String, val: f64) -> Result<()> {
        self.conn.set_ex(key, val, CACHE_TTL).map_err(|e| anyhow!("failed to write to cache: {}", e))
    }
}

pub struct InfuraClient{
    api_key: String,
    cache: Option<Box<dyn Cache<String, f64>>>,
}

impl InfuraClient {
    pub fn new() -> Result<InfuraClient> {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis-service:6379".to_string());
        let cache = match RedisCache::new(redis_url.as_str()) {
            Ok(cache) => {
                let c: Option<Box<dyn Cache<String, f64>>> = Some(Box::new(cache));
                c
            },
            Err(e) => {
                eprintln!("Failed to connect to redis: {}", e);
                None
            }
        };
        Ok(InfuraClient {
            api_key: std::env::var("INFURA_API_KEY").with_context(|| "couldn't get API key from environment")?,
            cache,
        })
    }

    pub async fn get_balance(&mut self, address: &str) -> Result<f64> {
        // try the cache first
        if let Some(cache) = &mut self.cache {
            if let Some(balance) = cache.read(address.to_string()).await? {
                CACHE_HIT_COUNT_METRIC.inc();
                return Ok(balance);
            }
        }
        API_ENDPOINT_REQUEST_COUNT_METRIC.inc();
        let http_client = reqwest::Client::new();
        let resp = http_client.post(format!("{}/{}", INFURA_ADDR, self.api_key))
            .body(serde_json::to_string(&GetRpcRequestBody::new(address, "eth_getBalance"))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("request failed with status code {}", resp.status()));
        }
        let resp_body: GetEthBalanceResp = resp.json().await?;

        if let Some(cache) = &mut self.cache {
            cache.write(address.to_string(), resp_body.balance_to_eth()?).await?;
        }

        Ok(resp_body.balance_to_eth()?)
    }

    pub async fn get_transaction(&mut self, transaction_hash: &str) -> Result<f64> {
        let http_client = reqwest::Client::new();
        let resp = http_client.post(format!("{}/{}", INFURA_ADDR, self.api_key))
            .body(serde_json::to_string(&GetRpcRequestBody::new(transaction_hash, "eth_getTransactionByHash"))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("request failed with status code {}", resp.status()));
        }
        // let resp_body = resp.json().await;
        Ok(0.)
    }
}


#[derive(Serialize)]
struct GetRpcRequestBody {
    jsonrpc: String,
    method: String,
    params: Vec<String>,
    id: i32,
}

impl GetRpcRequestBody {
    pub fn new(address: &str, method: &str) -> GetRpcRequestBody {
        GetRpcRequestBody {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: vec![address.to_string(), "latest".to_string()],
            id: 1,
        }
    }
}

#[derive(Deserialize, Debug)]
struct GetEthBalanceResp {
    jsonrpc: String,
    id: i32,
    result: String
}

impl GetEthBalanceResp {
    pub fn balance_to_eth(&self) -> Result<f64> {
        let result = self.result.strip_prefix("0x");
        match result {
            None => {
                Err(anyhow!("string is not in hex format (prefixed with 0x)"))
            },
            Some(hex_str) => match i128::from_str_radix(hex_str, 16) {
                Ok(balance) => Ok(balance as f64 / 1e18),
                Err(e) => {
                    Err(anyhow!("couldn't parse the wei hex value into an i64: {}", e))
                }
            }
        }
    }
}
