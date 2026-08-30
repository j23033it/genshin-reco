use thiserror::Error;
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourcePolicyError {
    #[error("根拠URLを解析できません")]
    InvalidUrl,
    #[error("根拠URLはHTTPSである必要があります")]
    HttpsRequired,
    #[error("根拠URLにuserinfoを含められません")]
    UserInfoNotAllowed,
    #[error("許可されていない根拠ホストです: {0}")]
    HostNotAllowed(String),
    #[error("許可されたサイト配下のURLではありません")]
    PathNotAllowed,
    #[error("根拠URLに非標準ポートを含められません")]
    PortNotAllowed,
}

pub fn normalize_source_url(raw: &str) -> Result<String, SourcePolicyError> {
    let mut url = Url::parse(raw).map_err(|_| SourcePolicyError::InvalidUrl)?;
    if url.scheme() != "https" {
        return Err(SourcePolicyError::HttpsRequired);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(SourcePolicyError::UserInfoNotAllowed);
    }
    if url.port().is_some_and(|port| port != 443) {
        return Err(SourcePolicyError::PortNotAllowed);
    }

    let host = url.host_str().ok_or(SourcePolicyError::InvalidUrl)?;
    match host {
        "wikiwiki.jp" if path_is_within(url.path(), "/genshinwiki") => {}
        "game8.jp" if path_is_within(url.path(), "/genshin") => {}
        "wiki.hoyolab.com" => {}
        "wikiwiki.jp" | "game8.jp" => return Err(SourcePolicyError::PathNotAllowed),
        other => return Err(SourcePolicyError::HostNotAllowed(other.into())),
    }

    // ページ内fragmentは根拠ページの同一性へ含めず、host側locatorで管理する。
    url.set_fragment(None);
    Ok(url.into())
}

fn path_is_within(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 許可された3サイトを正規化できる() {
        assert_eq!(
            normalize_source_url("https://wikiwiki.jp/genshinwiki/雷電将軍#build").unwrap(),
            "https://wikiwiki.jp/genshinwiki/%E9%9B%B7%E9%9B%BB%E5%B0%86%E8%BB%8D"
        );
        assert!(normalize_source_url("https://game8.jp/genshin/12345").is_ok());
        assert!(normalize_source_url("https://wiki.hoyolab.com/pc/genshin/entry/1").is_ok());
    }

    #[test]
    fn 偽装hostと危険なschemeと範囲外pathを拒否する() {
        for url in [
            "https://wikiwiki.jp.evil.example/genshinwiki/a",
            "javascript:alert(1)",
            "file:///C:/secret",
            "https://user@game8.jp/genshin/1",
            "https://game8.jp/other/1",
            "https://wikiwiki.jp/genshinwiki.evil/1",
            "https://wiki.hoyolab.com:444/pc/genshin/entry/1",
        ] {
            assert!(
                normalize_source_url(url).is_err(),
                "{url}を拒否できませんでした"
            );
        }
    }
}
