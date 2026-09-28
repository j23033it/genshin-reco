use crate::{
    catalog::load_embedded_catalog,
    domain::{AnalysisInput, AnalysisMode},
    source_policy::{is_direct_content_url, normalize_source_url},
};
use keyring::{Entry, Error as KeyringError};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use thiserror::Error;
use tokio::sync::Semaphore;

const KEYRING_SERVICE: &str = "jp.taiki.genshinreco";
const KEYRING_USER: &str = "tavily-api-key";
const SEARCH_ENDPOINT: &str = "https://api.tavily.com/search";
const EXTRACT_ENDPOINT: &str = "https://api.tavily.com/extract";
const ALLOWED_DOMAINS: [&str; 4] = ["wiki.hoyolab.com", "game8.jp", "wikiwiki.jp", "gamewith.jp"];
const MAX_EXTRACT_URLS: usize = 8;
const MAX_DIRECT_EXTRACT_URLS: usize = 20;
const MAX_PAGE_CHARS: usize = 8_000;
static TAVILY_REQUEST_LIMIT: Semaphore = Semaphore::const_new(4);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TavilySettingsStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TavilyExtractedPage {
    pub source_url: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Error)]
pub(crate) enum TavilyError {
    #[error("Tavily APIキーの保存先を利用できません: {0}")]
    CredentialStore(String),
    #[error("Tavily APIキーが設定されていません")]
    NotConfigured,
    #[error("Tavily APIキーを入力してください")]
    EmptyApiKey,
    #[error("Tavily APIへの接続に失敗しました: {0}")]
    Request(String),
    #[error("Tavily APIがエラーを返しました ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("Tavily APIの応答形式が不正です: {0}")]
    InvalidResponse(String),
    #[error("カタログを読み込めません: {0}")]
    Catalog(String),
}

#[derive(Serialize)]
struct SearchRequest<'a> {
    query: &'a str,
    topic: &'static str,
    search_depth: &'static str,
    max_results: u8,
    include_domains: &'a [&'a str],
    include_answer: bool,
    include_raw_content: bool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    title: String,
    url: String,
    #[serde(default)]
    score: f64,
}

#[derive(Serialize)]
struct ExtractRequest<'a> {
    urls: &'a [String],
    query: &'a str,
    extract_depth: &'static str,
    chunks_per_source: u8,
    include_images: bool,
}

#[derive(Debug, Deserialize)]
struct ExtractResponse {
    #[serde(default)]
    results: Vec<ExtractResult>,
}

#[derive(Debug, Deserialize)]
struct ExtractResult {
    url: String,
    raw_content: String,
}

#[tauri::command]
pub async fn read_tavily_settings_status() -> Result<TavilySettingsStatus, String> {
    run_credential_task(|| match read_api_key() {
        Ok(_) => Ok(TavilySettingsStatus { configured: true }),
        Err(TavilyError::NotConfigured) => Ok(TavilySettingsStatus { configured: false }),
        Err(error) => Err(error),
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_tavily_api_key(api_key: String) -> Result<TavilySettingsStatus, String> {
    let api_key = normalized_api_key(&api_key).map_err(|error| error.to_string())?;
    run_credential_task(move || {
        credential_entry()?
            .set_password(&api_key)
            .map_err(credential_error)?;
        Ok(TavilySettingsStatus { configured: true })
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_tavily_api_key() -> Result<TavilySettingsStatus, String> {
    run_credential_task(|| {
        let entry = credential_entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(TavilySettingsStatus { configured: false }),
            Err(error) => Err(credential_error(error)),
        }
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn test_tavily_connection() -> Result<(), String> {
    let api_key = stored_api_key().await.map_err(|error| error.to_string())?;
    let client = http_client().map_err(|error| error.to_string())?;
    search(
        &client,
        &api_key,
        "原神 キャラクター 聖遺物",
        AnalysisMode::Fast,
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

pub(crate) async fn fetch_research_context(
    analysis_input: &AnalysisInput,
    character_id: &str,
    mode: AnalysisMode,
) -> Result<Vec<TavilyExtractedPage>, TavilyError> {
    let api_key = match stored_api_key().await {
        Ok(key) => key,
        Err(TavilyError::NotConfigured) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let catalog =
        load_embedded_catalog().map_err(|error| TavilyError::Catalog(error.to_string()))?;
    let member = analysis_input
        .members
        .iter()
        .find(|member| member.character_id == character_id)
        .ok_or_else(|| TavilyError::Catalog("調査対象メンバーがいません".into()))?;
    let character = catalog
        .characters
        .iter()
        .find(|character| character.id == member.character_id)
        .ok_or_else(|| TavilyError::Catalog("調査対象キャラクターがカタログにありません".into()))?;
    let weapon = catalog
        .weapons
        .iter()
        .find(|weapon| weapon.id == member.weapon_id)
        .ok_or_else(|| TavilyError::Catalog("対象武器がカタログにありません".into()))?;
    let party_names = analysis_input
        .members
        .iter()
        .filter_map(|member| {
            catalog
                .characters
                .iter()
                .find(|candidate| candidate.id == member.character_id)
                .map(|candidate| candidate.name.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ");
    let queries = [
        format!(
            "原神 {} 編成 {} 天賦 命ノ星座 元素共鳴 聖遺物 目標ステータス Ver.{}",
            character.name, party_names, analysis_input.game_version
        ),
        format!(
            "原神 {} 精錬{} 武器効果 ステータス",
            weapon.name, member.refinement
        ),
    ];
    let client = http_client()?;
    let (first, second) = tokio::join!(
        search(&client, &api_key, &queries[0], mode),
        search(&client, &api_key, &queries[1], mode),
    );
    let mut candidates = Vec::new();
    let mut last_error = None;
    for result in [first, second] {
        match result {
            Ok(results) => candidates.extend(results),
            Err(error) => last_error = Some(error),
        }
    }
    if candidates.is_empty() {
        if let Some(error) = last_error {
            return Err(error);
        }
        return Ok(Vec::new());
    }
    let selected = select_search_results(candidates);
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let extract_query = format!(
        "{}の聖遺物、目標ステータス、武器・天賦・命ノ星座・元素共鳴・編成効果の数値根拠",
        character.name
    );
    extract_pages(&client, &api_key, &selected, &extract_query).await
}

/// Codex出力で新たに参照された許可済み本文URLを、Tavilyで追加検証する。
pub(crate) async fn extract_source_urls(
    urls: &[String],
    query: &str,
) -> Result<Vec<TavilyExtractedPage>, TavilyError> {
    let api_key = match stored_api_key().await {
        Ok(key) => key,
        Err(TavilyError::NotConfigured) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut selected = HashMap::<String, SearchResult>::new();
    for raw_url in urls {
        let Ok(normalized) = normalize_source_url(raw_url) else {
            continue;
        };
        if !is_direct_content_url(&normalized).unwrap_or(false) {
            continue;
        }
        selected.entry(normalized.clone()).or_insert(SearchResult {
            title: "Codexが参照した追加根拠".into(),
            url: normalized,
            score: 1.0,
        });
    }
    let mut selected = selected.into_values().collect::<Vec<_>>();
    selected.sort_by(|left, right| left.url.cmp(&right.url));
    selected.truncate(MAX_DIRECT_EXTRACT_URLS);
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let compact_query = query.chars().take(350).collect::<String>();
    let client = http_client()?;
    extract_pages(&client, &api_key, &selected, &compact_query).await
}

async fn stored_api_key() -> Result<String, TavilyError> {
    run_credential_task(read_api_key).await
}

async fn run_credential_task<T, F>(task: F) -> Result<T, TavilyError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, TavilyError> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| TavilyError::CredentialStore(error.to_string()))?
}

fn credential_entry() -> Result<Entry, TavilyError> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(credential_error)
}

fn read_api_key() -> Result<String, TavilyError> {
    match credential_entry()?.get_password() {
        Ok(key) => normalized_api_key(&key),
        Err(KeyringError::NoEntry) => Err(TavilyError::NotConfigured),
        Err(error) => Err(credential_error(error)),
    }
}

fn credential_error(error: KeyringError) -> TavilyError {
    TavilyError::CredentialStore(error.to_string())
}

fn normalized_api_key(api_key: &str) -> Result<String, TavilyError> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        Err(TavilyError::EmptyApiKey)
    } else {
        Ok(trimmed.to_string())
    }
}

fn http_client() -> Result<Client, TavilyError> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("genshin-reco/0.1")
        .build()
        .map_err(|error| TavilyError::Request(error.to_string()))
}

async fn search(
    client: &Client,
    api_key: &str,
    query: &str,
    mode: AnalysisMode,
) -> Result<Vec<SearchResult>, TavilyError> {
    let _permit = TAVILY_REQUEST_LIMIT
        .acquire()
        .await
        .map_err(|error| TavilyError::Request(error.to_string()))?;
    let body = SearchRequest {
        query,
        topic: "general",
        search_depth: if mode == AnalysisMode::Fast {
            "fast"
        } else {
            "basic"
        },
        max_results: 5,
        include_domains: &ALLOWED_DOMAINS,
        include_answer: false,
        include_raw_content: false,
    };
    let response = client
        .post(SEARCH_ENDPOINT)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|error| TavilyError::Request(error.to_string()))?;
    decode_json_response(response)
        .await
        .map(|response: SearchResponse| response.results)
}

fn select_search_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut selected = HashMap::<String, SearchResult>::new();
    for result in results {
        if result.score < 0.25 {
            continue;
        }
        let Ok(normalized) = normalize_source_url(&result.url) else {
            continue;
        };
        if !is_direct_content_url(&normalized).unwrap_or(false) {
            continue;
        }
        let replace = selected
            .get(&normalized)
            .is_none_or(|current| result.score > current.score);
        if replace {
            selected.insert(
                normalized.clone(),
                SearchResult {
                    url: normalized,
                    ..result
                },
            );
        }
    }
    let mut selected = selected.into_values().collect::<Vec<_>>();
    selected.sort_by(|left, right| right.score.total_cmp(&left.score));
    selected.truncate(MAX_EXTRACT_URLS);
    selected
}

async fn extract_pages(
    client: &Client,
    api_key: &str,
    selected: &[SearchResult],
    query: &str,
) -> Result<Vec<TavilyExtractedPage>, TavilyError> {
    let _permit = TAVILY_REQUEST_LIMIT
        .acquire()
        .await
        .map_err(|error| TavilyError::Request(error.to_string()))?;
    let urls = selected
        .iter()
        .map(|result| result.url.clone())
        .collect::<Vec<_>>();
    let titles = selected
        .iter()
        .map(|result| (result.url.as_str(), result.title.as_str()))
        .collect::<HashMap<_, _>>();
    let response = client
        .post(EXTRACT_ENDPOINT)
        .bearer_auth(api_key)
        .json(&ExtractRequest {
            urls: &urls,
            query,
            extract_depth: "basic",
            chunks_per_source: 3,
            include_images: false,
        })
        .send()
        .await
        .map_err(|error| TavilyError::Request(error.to_string()))?;
    let response: ExtractResponse = decode_json_response(response).await?;
    let mut pages = Vec::new();
    for result in response.results {
        let normalized = normalize_source_url(&result.url)
            .map_err(|error| TavilyError::InvalidResponse(error.to_string()))?;
        let content = result.raw_content.trim();
        if content.is_empty() || !titles.contains_key(normalized.as_str()) {
            continue;
        }
        pages.push(TavilyExtractedPage {
            title: titles
                .get(normalized.as_str())
                .copied()
                .unwrap_or("本文ページ")
                .to_string(),
            source_url: normalized,
            content: content.chars().take(MAX_PAGE_CHARS).collect(),
        });
    }
    Ok(pages)
}

async fn decode_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, TavilyError> {
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_else(|_| {
            status
                .canonical_reason()
                .unwrap_or("不明なエラー")
                .to_string()
        });
        return Err(TavilyError::Api {
            status: status.as_u16(),
            message: compact_error_message(status, &message),
        });
    }
    response
        .json::<T>()
        .await
        .map_err(|error| TavilyError::InvalidResponse(error.to_string()))
}

fn compact_error_message(status: StatusCode, message: &str) -> String {
    let compact = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("不明なエラー")
            .to_string()
    } else {
        compact.chars().take(300).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keyは前後空白を除去する() {
        assert_eq!(normalized_api_key("  tvly-test  ").unwrap(), "tvly-test");
        assert_eq!(
            normalized_api_key(" \n ").unwrap_err().to_string(),
            "Tavily APIキーを入力してください"
        );
    }

    #[test]
    fn 検索候補は許可済み本文urlだけをスコア順に残す() {
        let selected = select_search_results(vec![
            SearchResult {
                title: "低スコア".into(),
                url: "https://game8.jp/genshin/123".into(),
                score: 0.1,
            },
            SearchResult {
                title: "本文".into(),
                url: "https://game8.jp/genshin/99999#build".into(),
                score: 0.8,
            },
            SearchResult {
                title: "検索一覧".into(),
                url: "https://game8.jp/genshin/search?q=test".into(),
                score: 0.9,
            },
        ]);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].url, "https://game8.jp/genshin/99999");
    }
}
