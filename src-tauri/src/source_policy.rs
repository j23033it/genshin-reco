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
    // 数字のIDにも一覧ページがある。実際の攻略メニューで確認したものは
    // 検索の入口には使えても、装備効果の個別本文候補には含めない。
    if game == GameId::StarRail && is_star_rail_navigation(url.host_str(), path) {
        return Ok(false);
    }
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

fn is_star_rail_navigation(host: Option<&str>, path: &str) -> bool {
    let id = path.rsplit('/').next().unwrap_or("");
    match host {
        Some("game8.jp") => [
            "522613", "523707", "523754", "523971", "523977", "523983", "524339", "524356",
            "524357", "524359", "524364", "524365", "524366", "524367", "524368", "524369",
            "524370", "524371", "524372", "524373", "524374", "524375", "524376", "524662",
            "524663", "524664", "524665", "524666", "524667", "524668", "524699", "524705",
            "524706", "524795", "524880", "524988", "525010", "525140", "525208", "525210",
            "525211", "525291", "525329", "525364", "525405", "525406", "525616", "619853",
            "620483", "620491", "620614", "620773", "621061", "649313", "650087", "650951",
            "653235", "654569", "663497", "686477", "698597", "704297", "736857", "736858",
            "736859", "754020", "760838", "761469", "764130", "764148",
        ]
        .contains(&id),
        Some("gamewith.jp") => [
            "387676", "387751", "387752", "387753", "392812", "392916", "392917", "392918",
            "392919", "392920", "392921", "392922", "392923", "392924", "392925", "392926",
            "392927", "392928", "392929", "392930", "392931", "394481", "395118", "396232",
            "396257", "396729", "407940", "422627", "432939", "437010", "438009", "482015",
            "483233", "483675", "484221", "484344", "484646", "538540", "545450", "545542",
        ]
        .contains(&id),
        _ => false,
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
    fn 数字番号の一覧を拒否して実在する個別本文と公式wiki本文を許可する() {
        for url in [
            "https://game8.jp/houkaistarrail/523971",
            "https://game8.jp/houkaistarrail/525616#ornament",
            "https://gamewith.jp/houkaistarrail/article/show/387753",
            "https://gamewith.jp/houkaistarrail/article/show/394481",
        ] {
            assert!(
                !is_direct_content_url_for(GameId::StarRail, url).unwrap(),
                "{url}"
            );
        }
        for url in [
            "https://game8.jp/houkaistarrail/613642",
            "https://game8.jp/houkaistarrail/524823",
            "https://wiki.hoyolab.com/pc/hsr/entry/1537?crawler=Googlebot",
        ] {
            assert!(
                is_direct_content_url_for(GameId::StarRail, url).unwrap(),
                "{url}"
            );
        }
        assert!(
            is_direct_content_url_for(GameId::Genshin, "https://game8.jp/genshin/523971").unwrap()
        );
    }
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
