#Requires -Version 7.4
<#
skill-views.ps1 — operations 技能视图字节守卫(2026-10-10 con-06,G002 式)

权威源 = docs/operations/skills/<名>/;视图 = plugins/evo-explore/skills/<名>/(真文件,
市场安装拷目录)与 .claude/skills/<名>(符号链接,指向权威源)。check 罩全部 operations
技能:符号链接目标与字节、插件视图逐文件字节比对 + 无多余文件;退出码 0/1。fix 从权威源
拷贝后自检。

用法:pwsh tools/skill-views.ps1 check|fix
#>
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('check', 'fix')][string]$Mode)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

$repo = Split-Path $PSScriptRoot -Parent
$ops = Join-Path $repo 'docs/operations/skills'
$pluginView = Join-Path $repo 'plugins/evo-explore/skills'
$linkRoot = Join-Path $repo '.claude/skills'
$fail = 0

if (-not (Test-Path $ops)) { Write-Host "无 operations 技能目录: $ops"; exit 1 }
$skills = Get-ChildItem $ops -Directory | Sort-Object Name
if (-not $skills) { Write-Host "operations 零技能,无视图面"; exit 0 }

foreach ($s in $skills) {
    $name = $s.Name
    # ① 插件视图:逐文件字节等 + 无多余
    $dst = Join-Path $pluginView $name
    if ($Mode -eq 'fix') {
        New-Item -ItemType Directory -Force -Path $dst | Out-Null
        Get-ChildItem $s.FullName -Recurse -File | ForEach-Object {
            $rel = $_.FullName.Substring($s.FullName.Length + 1)
            $target = Join-Path $dst $rel
            New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
            Copy-Item $_.FullName $target -Force
        }
    }
    if (-not (Test-Path $dst)) {
        Write-Host "FAIL 插件视图缺: $name"; $fail = 1; continue
    }
    $srcFiles = Get-ChildItem $s.FullName -Recurse -File | ForEach-Object { $_.FullName.Substring($s.FullName.Length + 1) } | Sort-Object
    $dstFiles = Get-ChildItem $dst -Recurse -File | ForEach-Object { $_.FullName.Substring($dst.Length + 1) } | Sort-Object
    $missing = @($srcFiles | Where-Object { $_ -notin $dstFiles })
    $extra = @($dstFiles | Where-Object { $_ -notin $srcFiles })
    if ($missing) { Write-Host "FAIL 视图缺文件: $name -> $($missing -join ', ')"; $fail = 1 }
    if ($extra) { Write-Host "FAIL 视图多余文件: $name -> $($extra -join ', ')"; $fail = 1 }
    foreach ($rel in $srcFiles) {
        if ($rel -in $dstFiles) {
            $a = [System.IO.File]::ReadAllBytes((Join-Path $s.FullName $rel))
            $b = [System.IO.File]::ReadAllBytes((Join-Path $dst $rel))
            if ($a.Length -ne $b.Length -or (Compare-Object $a $b -SyncWindow 0)) {
                Write-Host "FAIL 字节不等: $name/$rel"; $fail = 1
            }
        }
    }
    # ② 符号链接视图:目标正确
    $link = Join-Path $linkRoot $name
    if (Test-Path $link) {
        $it = Get-Item $link
        if (-not ($it.LinkType -eq 'SymbolicLink' -and $it.Target -and (@($it.Target)[0] -like "*docs/operations/skills/$name"))) {
            Write-Host "FAIL 符号链接形错: $link -> $($it.Target)"; $fail = 1
        }
    }
}

if ($fail) { Write-Host 'skill-views: FAIL(退出码 1)'; exit 1 }
Write-Host "skill-views: OK($(@($skills).Count) 技能,视图全等)"; exit 0
