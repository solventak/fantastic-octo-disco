use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

static INFURA_ADDR: &str = "https://mainnet.infura.io/v3";

pub struct InfuraClient{
    api_key: String,
}

#[derive(Serialize)]
struct GetEthBalanceBody {
    jsonrpc: String,
    method: String,
    params: Vec<String>,
    id: i32,
}

impl GetEthBalanceBody {
    pub fn new(address: &str) -> GetEthBalanceBody {
        GetEthBalanceBody {
            jsonrpc: "2.0".to_string(),
            method: "eth_getBalance".to_string(),
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

impl InfuraClient {
    pub fn new() -> Result<InfuraClient> {
        Ok(InfuraClient {
            api_key: std::env::var("INFURA_API_KEY").with_context(|| "couldn't get API key from environment")?,
        })
    }

    pub async fn get_balance(&self, address: &str) -> Result<f64> {
        let http_client = reqwest::Client::new();
        let resp = http_client.post(format!("{}/{}", INFURA_ADDR, self.api_key))
            .body(serde_json::to_string(&GetEthBalanceBody::new(address))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("request failed with status code {}", resp.status()));
        }
        let resp_body: GetEthBalanceResp = resp.json().await?;
        resp_body.balance_to_eth()
    }
}
