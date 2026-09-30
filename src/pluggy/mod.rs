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
        values = parse_env_file(&text);
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

/// `KEY=value` lines; `#` comments and quotes around a value are ignored.
fn parse_env_file(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            values.insert(key.trim().to_string(), value.to_string());
        }
    }
    values
}

/// A value as it will sit on its own `KEY=value` line. Anything the parser would trim or
/// misread is refused rather than written into a file that then fails to load.
fn env_value<'a>(label: &str, value: &'a str) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() || PLACEHOLDERS.contains(&value) {
        return Err(format!("{} is required.", label));
    }
    if value
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '\'')
    {
        return Err(format!("{} can't contain spaces or quotes.", label));
    }
    Ok(value)
}

/// The contents of `pluggy.env` for these credentials. Item ids may be separated by commas or
/// spaces; they are written comma-separated, without duplicates.
pub fn credentials_file(
    client_id: &str,
    client_secret: &str,
    item_ids: &str,
) -> Result<String, String> {
    let client_id = env_value("Client ID", client_id)?;
    let client_secret = env_value("Client Secret", client_secret)?;
    let mut items: Vec<&str> = Vec::new();
    for id in item_ids.split(|c: char| c == ',' || c.is_whitespace()) {
        if id.is_empty() {
            continue;
        }
        let id = env_value("Item ID", id)?;
        if !items.contains(&id) {
            items.push(id);
        }
    }
    if items.is_empty() {
        return Err("At least one Item ID is required.".to_string());
    }
    Ok(format!(
        "PLUGGY_CLIENT_ID={}\nPLUGGY_CLIENT_SECRET={}\nPLUGGY_ITEM_IDS={}\n",
        client_id,
        client_secret,
        items.join(",")
    ))
}

/// Writes `pluggy.env` readable only by the user, replacing any existing one in one step so a
/// failed write never leaves half a file behind.
pub fn save_credentials(
    client_id: &str,
    client_secret: &str,
    item_ids: &str,
) -> Result<PathBuf, String> {
    let contents = credentials_file(client_id, client_secret, item_ids)?;
    let path = env_file_path().ok_or("Could not find the config directory.")?;
    let fail = |err: std::io::Error| format!("Could not write {}: {}", path.display(), err);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let tmp = path.with_extension("env.tmp");
    // The mode only applies on creation, so a leftover temp file must not be reused.
    let _ = std::fs::remove_file(&tmp);
    let written = write_private(&tmp, &contents).and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(err));
    }
    Ok(path)
}

fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(contents.as_bytes())
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Investment {
    /// EQUITY, ETF, FIXED_INCOME, MUTUAL_FUND, SECURITY, COE or OTHER.
    #[serde(rename = "type")]
    pub kind: String,
    pub balance: Option<Decimal>,
}

/// Everything one sync needs, fetched up front so the database write happens in one go.
pub struct SyncBatch {
    pub transactions: Vec<Transaction>,
    pub categories: HashMap<String, Category>,
    pub investments: Vec<Investment>,
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
/// configured items, plus their current investment holdings.
pub fn fetch(credentials: &Credentials, since: NaiveDate) -> Result<SyncBatch, String> {
    let client = Client::connect(credentials)?;

    let categories = client
        .get::<Page<Category>>("/categories")?
        .results
        .into_iter()
        .map(|category| (category.id.clone(), category))
        .collect();

    let mut transactions = Vec::new();
    let mut investments = Vec::new();
    for item_id in &credentials.item_ids {
        investments.extend(
            client
                .get::<Page<Investment>>(&format!("/investments?itemId={}", item_id))?
                .results,
        );
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
        investments,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_file_round_trips_through_the_parser() {
        let text = credentials_file(" id-1 ", "secret-1", "item-a, item-b item-a,").unwrap();
        assert_eq!(
            text,
            "PLUGGY_CLIENT_ID=id-1\nPLUGGY_CLIENT_SECRET=secret-1\nPLUGGY_ITEM_IDS=item-a,item-b\n"
        );
        let values = parse_env_file(&text);
        assert_eq!(values["PLUGGY_CLIENT_ID"], "id-1");
        assert_eq!(values["PLUGGY_CLIENT_SECRET"], "secret-1");
        assert_eq!(values["PLUGGY_ITEM_IDS"], "item-a,item-b");
    }

    #[test]
    fn credentials_file_refuses_values_the_parser_would_mangle() {
        assert!(credentials_file("", "secret", "item").is_err());
        assert!(credentials_file("id", "clientsecret-here", "item").is_err());
        assert!(credentials_file("id", "sec ret", "item").is_err());
        assert!(credentials_file("id", "secret\"", "item").is_err());
        assert!(credentials_file("id", "secret", " , ").is_err());
    }
}
