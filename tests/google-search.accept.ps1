#Requires -Version 7.4
<#
google-search.accept.ps1 — con-06 件3 验收实弹(2026-10-10)

连续 10 次实弹(隔离+串行:每跑前 browse down,引擎冷起,probe 常驻 profile 不变),
成功判据(件3 笺文) = results>=5 且 verdict=ok 且 drift=none;成功 >=8/10 即达标。
轨迹全量落盘 $OutDir(run-<n>.json + verdict-<n>.txt),退出码:0 = 达标,1 = 未达标,2 = 用法错。
判定纪律(gate-verdict):本脚本自身退出码即门禁判定,调用方勿再接吞退出码管道。
#>
[CmdletBinding()]
param(
    [string]$WorkspaceDir = "/mnt/wsl/repos/browse_workspace",
    [string]$OutDir = "/tmp/google-search-accept",
    [int]$Top = 5,
    [string[]]$Queries = @(
        'rust language', 'github actions cache', 'typescript generics tutorial',
        'postgresql jsonb index', 'linux io_uring', 'css container queries',
        'kubernetes sidecar pattern', 'sqlite wal mode', 'react server components',
        'dns over https privacy'
    )
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0
# 隔离铁律:本机若有他系 chrome(pi-rs 代理等)开着调试口,browse 附着优先会挂错浏览器
# (其 profile 无 probe → 注入恒缺,实锤 2026-10-11);NO_ATTACH 强制自起隔离实例
$env:BROWSE_NO_ATTACH = '1'

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$mech = Join-Path $WorkspaceDir 'page-skills/google-search/mech.rn'
if (-not (Test-Path $mech)) { throw "mech.rn 不存在: $mech" }

$results = @()
$i = 0
foreach ($q in $Queries) {
    $i++
    # 隔离:引擎每跑重起(daemon 常驻);down 后显式 up 暖机再测(治引擎冷起痉挛族:
    # 首导航竞引擎初始化致 CDP 帽尽/连接断),隔离语义不变(每跑全新引擎)
    & browse down *> $null
    Start-Sleep -Milliseconds 800
    & browse up --headless *> $null
    Start-Sleep -Seconds 1
    $qtag = ($q -replace ' ', '+')
    $runJson = Join-Path $OutDir "run-$i.json"
    $errFile = Join-Path $OutDir "run-$i.err"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    # 流分离 + 首个 { 截断解析:pwsh 流重定向怪癖会把 stderr 提示行混进文件头
    & browse rune $mech --vars "q=$qtag" --vars "top=$Top" --timeout 180 > $runJson 2> $errFile
    $rc = $LASTEXITCODE
    # 环境瞬态重试一次(down/up 引擎冷起痉挛族:navigate/evaluate 帽尽、引擎死亡、空输出;实录 err 侧,重试属同跑)
    if ($rc -ne 0) {
        Write-Host "run ${i}: rc=$rc transient (engine spawn churn), retrying"
        Start-Sleep -Seconds 1
        & browse rune $mech --vars "q=$qtag" --vars "top=$Top" --timeout 180 > $runJson 2>> $errFile
        $rc = $LASTEXITCODE
    }
    $sw.Stop()
    $verdict = $null; $drift = $null; $n = 0; $pages = 0; $settled = $null
    if ($rc -eq 0) {
        try {
            # pwsh 流重定向怪癖:stderr 提示行可混入文件头——从首个 { 截断再解析
            $raw = Get-Content $runJson -Raw
            $brace = $raw.IndexOf('{')
            if ($brace -gt 0) { $raw = $raw.Substring($brace) }
            $outer = $raw | ConvertFrom-Json
            $inner = $outer.ok | ConvertFrom-Json
            $verdict = $inner.probe.verdict
            $drift = $inner.probe.drift
            $n = @($inner.probe.results).Count
            $pages = @($inner.pages).Count
            $settled = $inner.probe.settled
        } catch { $verdict = "parse-error: $($_.Exception.Message.Substring(0, [Math]::Min(80, $_.Exception.Message.Length)))" }
    }
    $ok = ($rc -eq 0 -and $verdict -eq 'ok' -and $drift -eq 'none' -and $n -ge 5)
    $line = [pscustomobject]@{
        run = $i; query = $q; rc = $rc; ok = [int]$ok; verdict = $verdict
        drift = $drift; results = $n; pages = $pages; settled = $settled
        seconds = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    $results += $line
    $line | Format-Table -AutoSize | Out-String | Write-Host
}

$okCount = @($results | Where-Object { $_.ok -eq 1 }).Count
$results | Export-Csv (Join-Path $OutDir 'summary.csv') -NoTypeInformation
$summary = "accept: $okCount/10 ok (threshold >=8)"
Write-Host $summary
Write-Host ("summary-file: " + (Join-Path $OutDir 'summary.csv'))
if ($okCount -ge 8) { exit 0 } else { exit 1 }
