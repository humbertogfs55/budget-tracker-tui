//! Pluggy (Open Finance Brasil aggregator) client: pulls accounts, categories and
//! transactions for the configured items. Blocking; meant to run off the UI thread.

pub mod mapping;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

const API: &str = "https://api.pluggy.ai";
const ENV_FILE_NAME: &str = "pluggy.env";
const PLACEHOLDERS: [&str; 3] = ["clientid-here", "clientsecret-here", "itemid-here"];

pub struct Credentials {
    client_id: String,
    client_secret: String,
    item_ids: Vec<String>,
}

/// Where the credentials file lives, next to the app's config.json.
pub fn env_file_path() -> Option<PathBuf> {
    crate::config::app_config_dir()
        .ok()
        .map(|dir| dir.join(ENV_FILE_NAME))
}

/// Credentials from the environment, falling back to `pluggy.env`. `Ok(None)` means bank
/// sync simply isn't set up, which is the normal state for most installs.
pub fn load_credentials() -> Result<Option<Credentials>, String> {
    let mut values: HashMap<String, String> = HashMap::new();
    if let Some(path) = env_file_path().filter(|path| path.exists()) {
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("Could not read {}: {}", path.display(), err))?;
        for line in text.lines().map(str::trim) {
            if line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
                values.insert(key.trim().to_string(), value.to_string());
            }
        }
    }
    let mut get = |key: &str| {
        std::env::var(key)
            .ok()
            .or_else(|| values.remove(key))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && !PLACEHOLDERS.contains(&value.as_str()))
    };

    let (Some(client_id), Some(client_secret), Some(items)) = (
        get("PLUGGY_CLIENT_ID"),
        get("PLUGGY_CLIENT_SECRET"),
        get("PLUGGY_ITEM_IDS"),
    ) else {
        return Ok(None);
    };
    let mut item_ids: Vec<String> = Vec::new();
    for id in items.split(',').map(str::trim).filter(|id| !id.is_empty()) {
        if !item_ids.iter().any(|seen| seen == id) {
            item_ids.push(id.to_string());
        }
    }
    Ok(Some(Credentials {
        client_id,
        client_secret,
        item_ids,
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    /// RFC 3339 timestamp in UTC.
    pub date: String,
    pub description: String,
    /// Signed differently per account type; use `kind` for direction.
    pub amount: Decimal,
    /// "DEBIT" or "CREDIT".
    #[serde(rename = "type")]
    pub kind: String,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub description: String,
    pub parent_description: Option<String>,
}

/// Everything one sync needs, fetched up front so the database write happens in one go.
pub struct SyncBatch {
    pub transactions: Vec<Transaction>,
    pub categories: HashMap<String, Category>,
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Auth {
    api_key: String,
}

/// Fetch every transaction dated on or after `since` for all bank and credit accounts of the
/// configured items.
pub fn fetch(credentials: &Credentials, since: NaiveDate) -> Result<SyncBatch, String> {
    let client = Client::connect(credentials)?;

    let categories = client
        .get::<Page<Category>>("/categories")?
        .results
        .into_iter()
        .map(|category| (category.id.clone(), category))
        .collect();

    let mut transactions = Vec::new();
    for item_id in &credentials.item_ids {
        let accounts = client.get::<Page<Account>>(&format!("/accounts?itemId={}", item_id))?;
        for account in accounts.results {
            if account.kind != "BANK" && account.kind != "CREDIT" {
                continue;
            }
            transactions.extend(client.transactions(&account, since)?);
        }
    }

    Ok(SyncBatch {
        transactions,
        categories,
    })
}

struct Client {
    agent: ureq::Agent,
    api_key: String,
}

impl Client {
    fn connect(credentials: &Credentials) -> Result<Self, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            // Read error bodies ourselves: Pluggy explains failures there.
            .http_status_as_error(false)
            .build()
            .into();
        let response = agent
            .post(format!("{}/auth", API))
            .header("Accept", "application/json")
            .send_json(serde_json::json!({
                "clientId": credentials.client_id,
                "clientSecret": credentials.client_secret,
            }));
        let auth: Auth = read_json("/auth", response)?;
        Ok(Self {
            agent,
            api_key: auth.api_key,
        })
    }

    fn get<T: DeserializeOwned>(&self, path_and_query: &str) -> Result<T, String> {
        let response = self
            .agent
            .get(format!("{}{}", API, path_and_query))
            .header("Accept", "application/json")
            .header("X-API-KEY", &self.api_key)
            .call();
        read_json(path_and_query, response)
    }

    /// `GET /v2/transactions`, following the `next` cursor until it runs out.
    fn transactions(
        &self,
        account: &Account,
        since: NaiveDate,
    ) -> Result<Vec<Transaction>, String> {
        let first = first_page_query(account, since);
        let mut path = Some(first.clone());
        let mut out = Vec::new();
        while let Some(current) = path {
            let page: Page<Transaction> = self.get(&current)?;
            out.extend(page.results);
            path = page.next.map(|next| {
                // Documented as a ready-made query string; tolerate a full URL too.
                if let Some(rest) = next.strip_prefix(API) {
                    rest.to_string()
                } else if next.starts_with('?') {
                    format!("/v2/transactions{}", next)
                } else {
                    format!("{}&after={}", first, next)
                }
            });
        }
        Ok(out)
    }
}

fn first_page_query(account: &Account, since: NaiveDate) -> String {
    format!(
        "/v2/transactions?accountId={}&dateFrom={}",
        account.id,
        since.format("%Y-%m-%d")
    )
}

fn read_json<T: DeserializeOwned>(
    path: &str,
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<T, String> {
    // Only the path (never the query, which carries account ids) goes into error messages.
    let label = path.split('?').next().unwrap_or(path);
    let mut response = response.map_err(|err| format!("Pluggy {}: {}", label, err))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value.get("message")?.as_str().map(str::to_string))
            .unwrap_or(body);
        return Err(format!(
            "Pluggy {}: HTTP {} {}",
            label,
            status.as_u16(),
            message.chars().take(200).collect::<String>()
        ));
    }
    response
        .body_mut()
        .read_json::<T>()
        .map_err(|err| format!("Pluggy {}: unexpected response: {}", label, err))
}
