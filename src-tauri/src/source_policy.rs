use crate::game::GameId;
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
    normalize_source_url_for(GameId::Genshin, raw)
}

pub fn normalize_source_url_for(game: GameId, raw: &str) -> Result<String, SourcePolicyError> {
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
    let (wiki_root, guide_root, hoyo_game) = match game {
        GameId::Genshin => ("/genshinwiki", "/genshin", "genshin"),
        GameId::StarRail => ("/star-rail", "/houkaistarrail", "hsr"),
    };
    match host {
        "wikiwiki.jp" if path_is_within(url.path(), wiki_root) => {}
        "game8.jp" | "gamewith.jp" if path_is_within(url.path(), guide_root) => {}
        "wiki.hoyolab.com"
            if ["pc", "m"].iter().any(|platform| {
                path_is_within(url.path(), &format!("/{platform}/{hoyo_game}"))
            }) => {}
        "wikiwiki.jp" | "game8.jp" | "gamewith.jp" | "wiki.hoyolab.com" => {
            return Err(SourcePolicyError::PathNotAllowed);
        }
        other => return Err(SourcePolicyError::HostNotAllowed(other.into())),
    }

    // ページ内fragmentは根拠ページの同一性へ含めず、host側locatorで管理する。
    url.set_fragment(None);
    Ok(url.into())
}

/// 検索結果・サイトトップ・一覧ページではなく、個別本文ページとして扱えるURLか判定する。
///
/// URLだけで本文の存在を証明するものではないため、呼び出し側はWeb取得イベントの
/// `openPage`または`findInPage`と組み合わせて使用する。
pub fn is_direct_content_url(raw: &str) -> Result<bool, SourcePolicyError> {
    is_direct_content_url_for(GameId::Genshin, raw)
}

pub fn is_direct_content_url_for(game: GameId, raw: &str) -> Result<bool, SourcePolicyError> {
    let normalized = normalize_source_url_for(game, raw)?;
    let url = Url::parse(&normalized).map_err(|_| SourcePolicyError::InvalidUrl)?;
    let path = url.path().trim_end_matches('/');
    let has_search_query = url.query_pairs().any(|(key, _)| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "q" | "query" | "keyword" | "search" | "searchword"
        )
    });
    let has_navigation_action = url.query_pairs().any(|(key, value)| {
        key.eq_ignore_ascii_case("cmd")
            && matches!(value.as_ref(), "search" | "list" | "read" | "edit")
    });
    if has_search_query || has_navigation_action {
        return Ok(false);
    }

    let lower_path = path.to_ascii_lowercase();
    if lower_path
        .split('/')
        .any(|segment| matches!(segment, "search" | "list" | "category" | "recentchanges"))
    {
        return Ok(false);
    }

    let (wiki_root, guide_root) = match game {
        GameId::Genshin => ("/genshinwiki", "/genshin"),
        GameId::StarRail => ("/star-rail", "/houkaistarrail"),
    };
    match url.host_str() {
        Some("wikiwiki.jp") => {
            let decoded = percent_encoding::percent_decode_str(path)
                .decode_utf8()
                .map_err(|_| SourcePolicyError::InvalidUrl)?;
            let page = decoded.rsplit('/').next().unwrap_or("");
            let navigation = matches!(
                page.to_ascii_lowercase().as_str(),
                "frontpage" | "menubar" | "sidebar" | "sandbox" | "recentdeleted"
            ) || matches!(
                page,
                "キャラクター"
                    | "光円錐"
                    | "遺物"
                    | "オーナメント"
                    | "次元界オーナメント"
                    | "トンネル遺物"
                    | "武器"
                    | "聖遺物"
                    | "目次"
                    | "メニュー"
                    | "更新履歴"
            ) || page.ends_with("一覧")
                || page.ends_with("ランキング");
            Ok(path != wiki_root && !navigation)
        }
        Some("game8.jp") => Ok(path
            .strip_prefix(&format!("{guide_root}/"))
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))),
        Some("gamewith.jp") => Ok(path
            .strip_prefix(&format!("{guide_root}/article/show/"))
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))),
        Some("wiki.hoyolab.com") => Ok(path
            .split_once("/entry/")
            .is_some_and(|(_, id)| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))),
        _ => Ok(false),
    }
}

fn path_is_within(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 許可された攻略サイトを正規化できる() {
        assert_eq!(
            normalize_source_url("https://wikiwiki.jp/genshinwiki/雷電将軍#build").unwrap(),
            "https://wikiwiki.jp/genshinwiki/%E9%9B%B7%E9%9B%BB%E5%B0%86%E8%BB%8D"
        );
        assert!(normalize_source_url("https://game8.jp/genshin/12345").is_ok());
        assert_eq!(
            normalize_source_url("https://gamewith.jp/genshin/article/show/231920#weapon").unwrap(),
            "https://gamewith.jp/genshin/article/show/231920"
        );
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
            "https://gamewith.jp/other/article/show/1",
            "https://gamewith.jp/genshin.evil/article/show/1",
            "https://gamewith.jp.evil.example/genshin/article/show/1",
            "https://img.gamewith.jp/genshin/article/show/1",
            "https://wikiwiki.jp/genshinwiki.evil/1",
            "https://wiki.hoyolab.com:444/pc/genshin/entry/1",
        ] {
            assert!(
                normalize_source_url(url).is_err(),
                "{url}を拒否できませんでした"
            );
        }
    }

    #[test]
    fn 検索結果とトップを個別本文として扱わない() {
        for url in [
            "https://game8.jp/genshin/search?q=raiden",
            "https://game8.jp/genshin/",
            "https://gamewith.jp/genshin/",
            "https://gamewith.jp/genshin/article/show/",
            "https://gamewith.jp/genshin/article/show/search",
            "https://gamewith.jp/genshin/article/show/231920?q=test",
            "https://gamewith.jp/genshin/article/show/231920/extra",
            "https://wikiwiki.jp/genshinwiki/",
            "https://wiki.hoyolab.com/pc/genshin/home",
        ] {
            assert!(!is_direct_content_url(url).unwrap(), "{url}");
        }
        for url in [
            "https://game8.jp/genshin/12345",
            "https://gamewith.jp/genshin/article/show/231920",
            "https://wikiwiki.jp/genshinwiki/雷電将軍",
            "https://wiki.hoyolab.com/pc/genshin/entry/1",
        ] {
            assert!(is_direct_content_url(url).unwrap(), "{url}");
        }
    }
}

#[cfg(test)]
mod star_rail_tests {
    use super::*;
    #[test]
    fn スターレイル本文と原神を相互に分離する() {
        for url in [
            "https://wikiwiki.jp/star-rail/ホタル",
            "https://game8.jp/houkaistarrail/12345",
            "https://gamewith.jp/houkaistarrail/article/show/12345",
            "https://wiki.hoyolab.com/pc/hsr/entry/12345",
            "https://wiki.hoyolab.com/m/hsr/entry/12345",
        ] {
            assert!(
                is_direct_content_url_for(GameId::StarRail, url).unwrap(),
                "{url}"
            );
            assert!(
                normalize_source_url_for(GameId::Genshin, url).is_err(),
                "{url}"
            );
        }
        for url in [
            "https://wikiwiki.jp/genshinwiki/雷電将軍",
            "https://game8.jp/genshin/12345",
            "https://gamewith.jp/genshin/article/show/12345",
            "https://wiki.hoyolab.com/pc/genshin/entry/12345",
            "https://wiki.hoyolab.com/pc/zzz/entry/12345",
            "https://user@game8.jp/houkaistarrail/1",
            "https://game8.jp/houkaistarrail.evil/1",
            "https://game8.jp.evil.example/houkaistarrail/1",
            "https://wiki.hoyolab.com:444/pc/hsr/entry/1",
            "https://game8.jp/houkaistarrail/../genshin/1",
        ] {
            assert!(
                normalize_source_url_for(GameId::StarRail, url).is_err(),
                "{url}"
            );
        }
        for url in [
            "https://wikiwiki.jp/star-rail/",
            "https://wikiwiki.jp/star-rail/キャラクター",
            "https://wikiwiki.jp/star-rail/光円錐",
            "https://wikiwiki.jp/star-rail/%E9%81%BA%E7%89%A9",
            "https://wikiwiki.jp/star-rail/キャラクター一覧",
            "https://wikiwiki.jp/star-rail/MenuBar",
            "https://wikiwiki.jp/star-rail/ホタル?cmd=search",
            "https://game8.jp/houkaistarrail/",
            "https://game8.jp/houkaistarrail/list",
            "https://gamewith.jp/houkaistarrail/article/show/",
            "https://wiki.hoyolab.com/pc/hsr/home",
            "https://wiki.hoyolab.com/pc/hsr/entry/1?q=test",
            "https://wiki.hoyolab.com/pc/hsr/entry/search",
        ] {
            assert!(
                !is_direct_content_url_for(GameId::StarRail, url).unwrap(),
                "{url}"
            );
        }
    }
}
