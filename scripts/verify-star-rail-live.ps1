param(
    [Parameter(Mandatory)][string]$DatabasePath,
    [Parameter(Mandatory)][string]$CodexHome,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [string]$ResultDirectory,
    [string]$SessionId,
    [ValidateSet('mixed', 'fixed', 'cancel', 'failure', 'released', 'reuse', 'verify')]
    [string[]]$Cases = @('mixed', 'fixed', 'cancel', 'failure', 'released', 'reuse', 'verify')
)

# アプリの実調査を画面なしで呼ぶ。確認済みJSONの保存だけを検証するときは
# ResultDirectoryを指定する。通信中断の確認用ターンでは検索を無効にする。
# SQLで結果を作らず、アプリと共通の条件更新・検証・保存・再利用処理を呼ぶ。
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$DatabasePath = [IO.Path]::GetFullPath($DatabasePath)
$CodexHome = (Resolve-Path -LiteralPath $CodexHome).Path
$ReportDirectory = [IO.Path]::GetFullPath($ReportDirectory)
if (-not [string]::IsNullOrWhiteSpace($ResultDirectory)) { $ResultDirectory = (Resolve-Path -LiteralPath $ResultDirectory).Path }
[void][IO.Directory]::CreateDirectory($ReportDirectory)
$taskVariables = @('GENSHIN_RECO_INTAKE_PATH', 'GENSHIN_RECO_DATABASE_PATH', 'GENSHIN_RECO_CODEX_HOME',
    'GENSHIN_RECO_SESSION_ID', 'GENSHIN_RECO_STORE_MODE', 'GENSHIN_RECO_STORE_REPORT_PATH',
    'GENSHIN_RECO_LIVE_REPORT_PATH', 'GENSHIN_RECO_REPORT_PATH', 'GENSHIN_RECO_LIVE_FAULT',
    'GENSHIN_RECO_SINGLE_TURN', 'GENSHIN_RECO_EVIDENCE_ONLY', 'GENSHIN_RECO_SERVICE_PROFILE',
    'GENSHIN_RECO_SOURCE_PROFILE', 'GENSHIN_RECO_TEST_MODEL', 'GENSHIN_RECO_FINAL_EFFORT',
    'GENSHIN_RECO_FAILURE_MESSAGE', 'CARGO_TARGET_DIR')
$previousVariables = @{}
foreach ($name in $taskVariables) { $previousVariables[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$previousLocation = Get-Location

function Invoke-VerificationTest([string]$TestName, [string]$LogName) {
    $actualRoot = (& git rev-parse --show-toplevel).Trim()
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($actualRoot) -ne $projectRoot) { throw '作業場所が一致しません' }
    & (Join-Path $projectRoot 'scripts/use-cargo-target.ps1') | Out-Host
    & cargo test --manifest-path src-tauri/Cargo.toml --lib $TestName -- --ignored --nocapture *> (Join-Path $ReportDirectory "$LogName.log")
    if ($LASTEXITCODE -ne 0) { throw "確認に失敗しました: $LogName.log" }
}

function Invoke-Store([string]$Mode, [string]$Name) {
    $env:GENSHIN_RECO_STORE_MODE = $Mode
    $env:GENSHIN_RECO_STORE_REPORT_PATH = Join-Path $ReportDirectory "$Name.json"
    Invoke-VerificationTest '実調査の条件と結果を保存して再起動後に読み直せる' $Name
    Get-Content -LiteralPath $env:GENSHIN_RECO_STORE_REPORT_PATH -Raw | ConvertFrom-Json
}

try {
    Set-Location -LiteralPath $projectRoot
    $env:GENSHIN_RECO_DATABASE_PATH = $DatabasePath
    $env:GENSHIN_RECO_CODEX_HOME = $CodexHome
    $env:GENSHIN_RECO_SESSION_ID = if ([string]::IsNullOrWhiteSpace($SessionId)) { $null } else { $SessionId }
    $env:GENSHIN_RECO_SINGLE_TURN = $null
    $env:GENSHIN_RECO_EVIDENCE_ONLY = $null
    $env:GENSHIN_RECO_SERVICE_PROFILE = 'fast' # 製品の既定値。開発・レビューの実行設定とは別。
    $env:GENSHIN_RECO_SOURCE_PROFILE = 'gamewith'
    $env:GENSHIN_RECO_TEST_MODEL = $null
    $env:GENSHIN_RECO_FINAL_EFFORT = $null
    $names = @('ホタル', 'ルアン・メェイ', '開拓者・調和', 'ギャラガー')
    $cones = @('とある星神の殞落を記す', '記憶の中の姿', '輪契', '何が真か')
    foreach ($case in $Cases) {
        if ($case -eq 'reuse' -and -not [string]::IsNullOrWhiteSpace($ResultDirectory)) {
            $releasedResult = Get-Content -LiteralPath (Join-Path $ResultDirectory 'released.json') -Raw | ConvertFrom-Json
            if ($releasedResult.verificationKind -eq 'operator_verified_fixture') {
                @{ case = $case; skipped = $true; reason = '検証用出力を実調査キャッシュへ登録しないため' } |
                    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $ReportDirectory 'script-reuse-summary.json') -Encoding utf8NoBOM
                Write-Output 'reuse 除外: 作業者の検証用出力は実調査の再利用対象にしない'
                continue
            }
        }
        $started = [DateTimeOffset]::Now
        $members = @(0..3 | ForEach-Object {
            @{ slotIndex = $_; name = $names[$_]; weapon = $cones[$_]; constellation = 0; refinement = 1; relics = $null }
        })
        if ($case -eq 'fixed') {
            foreach ($member in $members) { $member.relics = @{ tunnel = @{ kind = 'four_piece'; set = '草の穂ガンマン' }; ornament = '折れた竜骨' } }
        } elseif ($case -eq 'mixed') {
            $members[0].relics = @{ tunnel = @{ kind = 'four_piece'; set = '草の穂ガンマン' }; ornament = '折れた竜骨' }
            $members[1].relics = @{ tunnel = @{ kind = 'two_plus_two'; sets = @('草の穂ガンマン', '流星の跡を追う怪盗') }; ornament = $null }
            $members[2].relics = @{ tunnel = $null; ornament = '盗賊公国タリア' }
            $members[3].relics = @{ tunnel = @{ kind = 'four_piece'; set = '草の穂ガンマン' }; ornament = $null }
        }
        $env:GENSHIN_RECO_INTAKE_PATH = Join-Path $ReportDirectory "$case-intake.json"
        @{ game = 'star_rail'; members = $members; missingFields = @(); readyToResearch = $true } |
            ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $env:GENSHIN_RECO_INTAKE_PATH -Encoding utf8NoBOM
        if ($case -in @('reuse', 'verify')) {
            $stored = Invoke-Store $case "script-$case"
            if ($case -eq 'reuse') {
                $originalSession = $env:GENSHIN_RECO_SESSION_ID
                $env:GENSHIN_RECO_SESSION_ID = $stored.sessionId
                $null = Invoke-Store 'verify' 'script-reuse-reopened'
                $env:GENSHIN_RECO_SESSION_ID = $originalSession
            }
        } else {
            $prepared = Invoke-Store 'prepare' "script-$case-prepared"
            $env:GENSHIN_RECO_SESSION_ID = $prepared.sessionId
            try {
            if ($case -in @('cancel', 'failure') -or [string]::IsNullOrWhiteSpace($ResultDirectory)) {
                $env:GENSHIN_RECO_LIVE_FAULT = if ($case -in @('cancel', 'failure')) { $case } else { $null }
                $env:GENSHIN_RECO_REPORT_PATH = Join-Path $ReportDirectory "script-$case-live.json"
                $env:GENSHIN_RECO_LIVE_REPORT_PATH = $env:GENSHIN_RECO_REPORT_PATH
                Invoke-VerificationTest '実codexで資料収集と編成検討を同じ会話で完了できる' "script-$case-live"
            } else {
                $env:GENSHIN_RECO_LIVE_REPORT_PATH = Join-Path $ResultDirectory "$case.json"
                if (-not (Test-Path -LiteralPath $env:GENSHIN_RECO_LIVE_REPORT_PATH -PathType Leaf)) { throw "確認済みの出力がありません: $case.json" }
            }
            $mode = if ($case -in @('cancel', 'failure')) { $case } else { 'save' }
            $stored = Invoke-Store $mode "script-$case-saved"
            } catch {
                # 実調査・検査が失敗しても、調査中のまま操作不能にしない。
                $env:GENSHIN_RECO_FAILURE_MESSAGE = $_.Exception.Message
                $null = Invoke-Store 'abort' "script-$case-aborted"
                throw
            }
            if ($mode -eq 'save') { $null = Invoke-Store 'verify' "script-$case-reopened" }
        }
        $summary = @{ case = $case; sessionId = $stored.sessionId; startedAt = $started.ToString('o'); elapsedSeconds = ([DateTimeOffset]::Now - $started).TotalSeconds; status = $stored.conversation.status; teamId = $stored.result.teamId }
        $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $ReportDirectory "script-$case-summary.json") -Encoding utf8NoBOM
        Write-Output "$case 完了: $($summary.elapsedSeconds.ToString('F1'))秒 / $($summary.status) / $($summary.teamId)"
    }
} finally {
    foreach ($name in $taskVariables) { [Environment]::SetEnvironmentVariable($name, $previousVariables[$name], 'Process') }
    Set-Location -LiteralPath $previousLocation.Path
}
