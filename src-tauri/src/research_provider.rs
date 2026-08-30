use crate::domain::{
    AnalysisInput, CharacterResearchOutput, DomainValidationError, validate_analysis_input,
    validate_character_research_output,
};
use std::{collections::HashMap, error::Error, fmt, future::Future, pin::Pin};

/// キャラクター1件の調査を依頼するための入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterResearchRequest {
    pub analysis_input: AnalysisInput,
    pub character_id: String,
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
            Self::CharacterNotConfigured { .. } | Self::Unsupported => None,
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
        ArtifactPlan, BuildCondition, BuildIntent, CharacterBuildIntent, EnergyPriority,
        EvidenceClaimType, FixedAssumptions, MainStatPackage, NormalizedClaimValue,
        PartyMemberInput, ReactionOwnership, ResearchBuildVariant, ResearchClaim, ResearchEvidence,
        ResearchLocator, ResearchSchemaVersion, ResearchSourcePage, StatPriority, StatUnit,
        SurvivabilityPriority, TargetScope, TargetStatRange,
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
                intent: CharacterBuildIntent {
                    role: BuildIntent::Auto,
                    reaction_ownership: ReactionOwnership::Unknown,
                    energy_priority: EnergyPriority::Balanced,
                    survivability_priority: SurvivabilityPriority::Normal,
                },
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
            target_stats: vec![TargetStatRange {
                stat: "攻撃力".to_string(),
                minimum: Some(1000.0),
                maximum: None,
                unit: StatUnit::Flat,
                scope: TargetScope::CharacterSheetUnbuffed,
                note: None,
            }],
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
                normalized_value: NormalizedClaimValue::ArtifactPlan {
                    value: ArtifactPlan::FourPiece {
                        set_id: "set-a".to_string(),
                    },
                },
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::MainStatPackage,
                normalized_value: NormalizedClaimValue::MainStatPackage {
                    value: package.clone(),
                },
                conditions: Vec::new(),
                evidence: evidence.clone(),
            },
            ResearchClaim {
                claim_type: EvidenceClaimType::SubstatPriority,
                normalized_value: NormalizedClaimValue::SubstatPriority {
                    value: package.substat_priority.clone(),
                },
                conditions: Vec::new(),
                evidence,
            },
        ];
        CharacterResearchOutput {
            schema_version: ResearchSchemaVersion::V1,
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
