use crate::app_server;
use crate::domain::{
    AnalysisInput, CharacterResearchOutput, DomainValidationError, EvidenceVerification,
    validate_analysis_input, validate_character_research_output,
};
use crate::hashing::sha256_canonical;
use crate::reconciler::VerifiedSourcePage;
use crate::source_policy::normalize_source_url;
use serde::Serialize;
use std::{collections::HashMap, error::Error, fmt, future::Future, pin::Pin};
use tauri::Manager;
use url::Url;

/// キャラクター1件の調査を依頼するための入力。
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterResearchRequest {
    pub analysis_input: AnalysisInput,
    pub character_id: String,
    pub prior_research: Option<CharacterResearchOutput>,
}

/// 調査プロバイダが返す非同期処理の型。
pub type ResearchFuture =
    Pin<Box<dyn Future<Output = Result<CharacterResearchOutput, ResearchProviderError>> + Send>>;

/// 調査処理で発生したエラー。
#[derive(Debug, PartialEq, Eq)]
pub enum ResearchProviderError {
    /// 分析入力がドメイン契約に違反している。
    InvalidAnalysisInput(DomainValidationError),
    /// 要求されたキャラクターのfixtureが登録されていない。
    CharacterNotConfigured { character_id: String },
    /// 登録済みfixtureがドメイン契約に違反している。
    InvalidFixture {
        character_id: String,
        source: DomainValidationError,
    },
    /// Codex App Serverによる実調査に失敗した。
    AppServer(String),
    /// 本文イベントから検証済み根拠を生成できなかった。
    EvidenceVerification(String),
    /// プロバイダ実装がどちらの入口も実装していない。
    Unsupported,
}

impl fmt::Display for ResearchProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAnalysisInput(source) => {
                write!(formatter, "分析入力が不正です: {source}")
            }
            Self::CharacterNotConfigured { character_id } => {
                write!(formatter, "調査fixtureが未設定です: {character_id}")
            }
            Self::InvalidFixture {
                character_id,
                source,
            } => write!(
                formatter,
                "調査fixtureが不正です ({character_id}): {source}"
            ),
            Self::AppServer(message) => write!(formatter, "Codex調査に失敗しました: {message}"),
            Self::EvidenceVerification(message) => {
                write!(formatter, "根拠検証に失敗しました: {message}")
            }
            Self::Unsupported => write!(formatter, "調査プロバイダが未実装です"),
        }
    }
}

impl Error for ResearchProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAnalysisInput(source) | Self::InvalidFixture { source, .. } => {
                Some(source)
            }
            Self::CharacterNotConfigured { .. }
            | Self::AppServer(_)
            | Self::EvidenceVerification(_)
            | Self::Unsupported => None,
        }
    }
}

/// 実際の調査実装とfixture実装を差し替えるための契約。
///
/// `research`を主入口とし、名前を明示したい利用側向けに
/// `research_character`も提供する。どちらか一方だけを実装した型でも利用できる。
pub trait ResearchProvider: Send + Sync {
    fn research(&self, request: CharacterResearchRequest) -> ResearchFuture {
        self.research_character(request)
    }

    fn research_character(&self, _request: CharacterResearchRequest) -> ResearchFuture {
        Box::pin(async { Err(ResearchProviderError::Unsupported) })
    }
}

/// 実Web調査の出力と、App Serverイベントからホストが検証したページ一覧。
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedCharacterResearch {
    pub output: CharacterResearchOutput,
    pub verified_pages: Vec<VerifiedSourcePage>,
}

/// 常駐Codex App Serverを利用する実調査プロバイダ。
#[derive(Clone)]
pub struct CodexResearchProvider {
    app: tauri::AppHandle,
}

impl CodexResearchProvider {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    pub fn research_verified(&self, request: CharacterResearchRequest) -> VerifiedResearchFuture {
        self.research_verified_cancellable(request, None)
    }

    pub(crate) fn research_verified_cancellable(
        &self,
        request: CharacterResearchRequest,
        cancellation: Option<app_server::ResearchCancellation>,
    ) -> VerifiedResearchFuture {
        let app = self.app.clone();
        Box::pin(async move {
            validate_analysis_input(&request.analysis_input)
                .map_err(ResearchProviderError::InvalidAnalysisInput)?;
            let supervisor = app.state::<app_server::AppServerSupervisor>();
            let observed = app_server::research_character_with_codex(
                &app,
                supervisor.inner(),
                &request.analysis_input,
                &request.character_id,
                cancellation.as_ref(),
                request.prior_research.as_ref(),
            )
            .await
            .map_err(ResearchProviderError::AppServer)?;
            let verified_pages = build_verified_pages(&observed.output)?;
            Ok(VerifiedCharacterResearch {
                output: observed.output,
                verified_pages,
            })
        })
    }
}

impl ResearchProvider for CodexResearchProvider {
    fn research(&self, request: CharacterResearchRequest) -> ResearchFuture {
        let provider = self.clone();
        Box::pin(async move {
            provider
                .research_verified(request)
                .await
                .map(|research| research.output)
        })
    }
}

pub type VerifiedResearchFuture =
    Pin<Box<dyn Future<Output = Result<VerifiedCharacterResearch, ResearchProviderError>> + Send>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapturedPageEvidence<'a> {
    source: &'a crate::domain::ResearchSourcePage,
    claims: Vec<&'a crate::domain::ResearchClaim>,
}

fn build_verified_pages(
    output: &CharacterResearchOutput,
) -> Result<Vec<VerifiedSourcePage>, ResearchProviderError> {
    let mut verified_pages = Vec::new();
    for source in &output.sources {
        let normalized = normalize_source_url(&source.source_url)
            .map_err(|error| ResearchProviderError::EvidenceVerification(error.to_string()))?;
        let claims = output
            .variants
            .iter()
            .flat_map(|variant| variant.claims.iter())
            .filter(|claim| {
                normalize_source_url(&claim.evidence.source_url)
                    .is_ok_and(|claim_url| claim_url == normalized)
            })
            .collect::<Vec<_>>();
        if claims.is_empty() {
            continue;
        }
        let content_hash = sha256_canonical(&CapturedPageEvidence { source, claims })
            .map_err(|error| ResearchProviderError::EvidenceVerification(error.to_string()))?;
        let source_page_hash = sha256_canonical(&normalized)
            .map_err(|error| ResearchProviderError::EvidenceVerification(error.to_string()))?;
        let host = Url::parse(&normalized)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .ok_or_else(|| {
                ResearchProviderError::EvidenceVerification(
                    "参照ページのhostを取得できません".into(),
                )
            })?;
        verified_pages.push(VerifiedSourcePage {
            source_url: normalized,
            source_page_id: format!("source-{source_page_hash}"),
            content_hash: Some(content_hash),
            verification: EvidenceVerification::HostExactMatch,
            source_family: host,
            is_direct_content_page: true,
        });
    }
    Ok(verified_pages)
}

/// 決められたfixtureを使う、ネットワーク不要の調査プロバイダ。
#[derive(Debug, Clone, Default)]
pub struct FakeResearchProvider {
    fixtures: HashMap<String, CharacterResearchOutput>,
}

impl FakeResearchProvider {
    /// キャラクターIDと調査結果のfixtureからプロバイダを作る。
    pub fn new<I>(fixtures: I) -> Self
    where
        I: IntoIterator<Item = (String, CharacterResearchOutput)>,
    {
        Self {
            fixtures: fixtures.into_iter().collect(),
        }
    }

    /// fixtureを明示的に設定してプロバイダを作る別名。
    pub fn from_fixtures<I>(fixtures: I) -> Self
    where
        I: IntoIterator<Item = (String, CharacterResearchOutput)>,
    {
        Self::new(fixtures)
    }

    /// fixtureを追加または置換する。
    pub fn set_fixture(&mut self, character_id: String, output: CharacterResearchOutput) {
        self.fixtures.insert(character_id, output);
    }

    /// 登録済みfixtureを読み取り専用で確認する。
    pub fn fixtures(&self) -> &HashMap<String, CharacterResearchOutput> {
        &self.fixtures
    }

    /// 登録済みfixtureを使って調査する。
    pub fn research(&self, request: CharacterResearchRequest) -> ResearchFuture {
        self.research_impl(request)
    }

    /// `research`のキャラクター明示名による別名。
    pub fn research_character(&self, request: CharacterResearchRequest) -> ResearchFuture {
        self.research_impl(request)
    }

    fn research_impl(&self, request: CharacterResearchRequest) -> ResearchFuture {
        let result = validate_analysis_input(&request.analysis_input)
            .map_err(ResearchProviderError::InvalidAnalysisInput)
            .and_then(|_| {
                let Some(output) = self.fixtures.get(&request.character_id) else {
                    return Err(ResearchProviderError::CharacterNotConfigured {
                        character_id: request.character_id.clone(),
                    });
                };

                validate_character_research_output(
                    output,
                    &request.character_id,
                    &request.analysis_input.game_version,
                )
                .map_err(|source| ResearchProviderError::InvalidFixture {
                    character_id: request.character_id.clone(),
                    source,
                })?;
                Ok(output.clone())
            });

        Box::pin(async move { result })
    }
}

impl ResearchProvider for FakeResearchProvider {
    fn research(&self, request: CharacterResearchRequest) -> ResearchFuture {
        FakeResearchProvider::research(self, request)
    }

    fn research_character(&self, request: CharacterResearchRequest) -> ResearchFuture {
        FakeResearchProvider::research_character(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ArtifactPlan, BuildCondition, EvidenceClaimType, FixedAssumptions, MainStatPackage,
        PartyMemberInput, ResearchBuildVariant, ResearchClaim, ResearchClaimValue,
        ResearchEvidence, ResearchLocator, ResearchSchemaVersion, ResearchSourcePage, StatPriority,
        StatUnit, TargetScope, TargetStatRange,
    };

    fn valid_input() -> AnalysisInput {
        AnalysisInput {
            party_id: "party-1".to_string(),
            party_name: "検証編成".to_string(),
            game_version: "7.0".to_string(),
            members: std::array::from_fn(|slot_index| PartyMemberInput {
                slot_index: slot_index as u8,
                character_id: format!("char-{slot_index}"),
                weapon_id: format!("weapon-{slot_index}"),
                refinement: 1,
                constellation: 0,
            }),
            assumptions: FixedAssumptions {
                character_level: 90,
                weapon_level: 90,
                artifact_level: 20,
                artifact_rarity: 5,
                sheet_timing: "pre_combat".to_string(),
                final_ascension: true,
                all_talents_available: true,
                witch_teaching_when_applicable: true,
            },
            versions: crate::domain::AnalysisVersions {
                catalog_version: "catalog-v1".to_string(),
                source_policy_version: "source-v1".to_string(),
                prompt_version: "prompt-v1".to_string(),
                schema_version: "schema-v1".to_string(),
                reconciler_version: "reconciler-v1".to_string(),
                solver_version: "solver-v1".to_string(),
            },
        }
    }

    fn valid_output(character_id: &str) -> CharacterResearchOutput {
        let source_url = "https://wikiwiki.jp/genshinwiki/example".to_string();
        let package = MainStatPackage {
            id: "main-1".to_string(),
            sands: "攻撃力%".to_string(),
            goblet: "元素ダメージ".to_string(),
            circlet: "会心率".to_string(),
            conditions: Vec::<BuildCondition>::new(),
            substat_priority: vec![StatPriority {
                stat: "会心率".to_string(),
                rank: 1,
            }],
            target_stats: vec![
                TargetStatRange {
                    stat: "攻撃力".to_string(),
                    minimum: Some(1000.0),
                    maximum: None,
                    unit: StatUnit::Flat,
                    scope: TargetScope::CharacterSheetUnbuffed,
                    included_bonuses: vec![],
                    note: Some("会心との配分を考慮した下限".to_string()),
                },
                TargetStatRange {
                    stat: "会心率".to_string(),
                    minimum: Some(60.0),
                    maximum: Some(85.0),
                    unit: StatUnit::Percent,
                    scope: TargetScope::CharacterSheetUnbuffed,
                    included_bonuses: vec![crate::domain::TargetStatBonus {
                        source: "氷共鳴".to_string(),
                        amount: 15.0,
                        condition: Some("氷元素付着中".to_string()),
                    }],
                    note: Some("戦闘中の加算込みで100%以下".to_string()),
                },
            ],
        };
        let evidence = ResearchEvidence {
            source_url: source_url.clone(),
            evidence_excerpt: None,
            evidence_summary: "検証用の要約".to_string(),
            locator: Some(ResearchLocator {
                heading: Some("ビルド".to_string()),
                section: None,
                text_fragment: None,
            }),
        };
        let claims = vec![
            ResearchClaim {
                claim_type: EvidenceClaimType::ArtifactPlan,
                normalized_value: ResearchClaimValue::ArtifactPlan,
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::MainStatPackage,
                normalized_value: ResearchClaimValue::MainStatPackage,
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::SubstatPriority,
                normalized_value: ResearchClaimValue::SubstatPriority,
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::TargetStat,
                normalized_value: ResearchClaimValue::TargetStat {
                    stat: package.target_stats[0].stat.clone(),
                    scope: package.target_stats[0].scope,
                },
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::TargetStat,
                normalized_value: ResearchClaimValue::TargetStat {
                    stat: package.target_stats[1].stat.clone(),
                    scope: package.target_stats[1].scope,
                },
                conditions: Vec::new(),
                evidence,
            },
        ];
        CharacterResearchOutput {
            schema_version: ResearchSchemaVersion::V2,
            character_id: character_id.to_string(),
            sources: vec![ResearchSourcePage {
                source_url,
                title: "検証ページ".to_string(),
                publisher: "原神 Wiki".to_string(),
                game_version: "7.0".to_string(),
                updated_at: None,
            }],
            variants: vec![ResearchBuildVariant {
                id: "variant-a".to_string(),
                artifact_plan: ArtifactPlan::FourPiece {
                    set_id: "set-a".to_string(),
                },
                main_stat_package: package,
                conditions: Vec::new(),
                team_buff_keys: Vec::new(),
                claims,
            }],
            warnings: Vec::new(),
        }
    }

    fn request(character_id: &str) -> CharacterResearchRequest {
        CharacterResearchRequest {
            analysis_input: valid_input(),
            character_id: character_id.to_string(),
            prior_research: None,
        }
    }

    #[test]
    fn 正常なfixtureを決定論的に返す() {
        let provider = FakeResearchProvider::new([("char-a".to_string(), valid_output("char-a"))]);
        let first = futures_block_on(provider.research(request("char-a"))).unwrap();
        let second = futures_block_on(provider.research_character(request("char-a"))).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn 未設定キャラクターは専用エラーになる() {
        let provider = FakeResearchProvider::default();
        let error = futures_block_on(provider.research(request("char-a"))).unwrap_err();
        assert_eq!(
            error,
            ResearchProviderError::CharacterNotConfigured {
                character_id: "char-a".to_string()
            }
        );
    }

    #[test]
    fn 不正な分析入力は入力エラーになる() {
        let mut input = valid_input();
        input.assumptions.character_level = 80;
        let provider = FakeResearchProvider::new([("char-a".to_string(), valid_output("char-a"))]);
        let error = futures_block_on(provider.research(CharacterResearchRequest {
            analysis_input: input,
            character_id: "char-a".to_string(),
            prior_research: None,
        }))
        .unwrap_err();
        assert!(matches!(
            error,
            ResearchProviderError::InvalidAnalysisInput(_)
        ));
    }

    #[test]
    fn 不正なfixtureはfixtureエラーになる() {
        let provider = FakeResearchProvider::new([("char-a".to_string(), valid_output("char-b"))]);
        let error = futures_block_on(provider.research(request("char-a"))).unwrap_err();
        assert!(matches!(
            error,
            ResearchProviderError::InvalidFixture { character_id, .. }
                if character_id == "char-a"
        ));
    }

    #[test]
    fn 分析対象と根拠のゲーム版不一致を拒否する() {
        let mut output = valid_output("char-a");
        output.sources[0].game_version = "6.0".into();
        let provider = FakeResearchProvider::new([("char-a".to_string(), output)]);

        let error = futures_block_on(provider.research(request("char-a"))).unwrap_err();

        assert!(matches!(
            error,
            ResearchProviderError::InvalidFixture { source, .. }
                if source.to_string().contains("ゲーム版")
        ));
    }

    #[test]
    fn 本文根拠から決定論的な検証済みページを作る() {
        let output = valid_output("char-a");

        let first = build_verified_pages(&output).expect("検証済みページを作れること");
        let second = build_verified_pages(&output).expect("再度作れること");

        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].verification, EvidenceVerification::HostExactMatch);
        assert!(first[0].is_direct_content_page);
        assert_eq!(first[0].source_family, "wikiwiki.jp");
        assert!(
            first[0]
                .content_hash
                .as_deref()
                .is_some_and(|hash| hash.len() == 64)
        );
        assert!(first[0].source_page_id.starts_with("source-"));
    }

    #[test]
    fn claimから参照されないsourceは無視する() {
        let mut output = valid_output("char-a");
        output.sources.push(ResearchSourcePage {
            source_url: "https://game8.jp/genshin/12345".into(),
            title: "未使用ページ".into(),
            publisher: "Game8".into(),
            game_version: "7.0".into(),
            updated_at: None,
        });

        let pages = build_verified_pages(&output).expect("未使用sourceを除外できること");
        assert_eq!(pages.len(), 1);
        assert_eq!(
            pages[0].source_url,
            "https://wikiwiki.jp/genshinwiki/example"
        );
    }

    fn futures_block_on<F: Future>(future: F) -> F::Output {
        use std::{
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
        };

        struct Noop;
        impl Wake for Noop {
            fn wake(self: Arc<Self>) {}
        }

        let waker = Waker::from(Arc::new(Noop));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
