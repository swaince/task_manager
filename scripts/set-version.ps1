<#
.SYNOPSIS
    把同一个版本号同步写入 package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json。

.DESCRIPTION
    版本号必须三处一致：前端产物（package.json）、Rust crate（Cargo.toml）、
    Tauri 打包元数据（tauri.conf.json，它决定安装包文件名与注册表项）。

    实现上只做「就地字符串替换」，不用 ConvertFrom-Json 重排整个文件，
    以保证 git diff 干净、注释与字段顺序不被破坏。

.PARAMETER Version
    目标版本，允许带前导 v（v1.0.0 与 1.0.0 等价），必须是 semver。

.PARAMETER RepoRoot
    仓库根目录，默认取本脚本的上一级目录。

.EXAMPLE
    pwsh scripts/set-version.ps1 -Version v1.0.0
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version,

    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

# $PSScriptRoot 在 `-File` 调用形式的参数默认值里可能为空，放到脚本体里求值。
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
}

# --- 归一化并校验版本号 -------------------------------------------------
$normalized = $Version.Trim()
if ($normalized.StartsWith('v') -or $normalized.StartsWith('V')) {
    $normalized = $normalized.Substring(1)
}
if ($normalized -notmatch '^\d+\.\d+\.\d+([-+][0-9A-Za-z.\-]+)?$') {
    throw "版本号 '$Version' 不是合法 semver（应形如 1.0.0 或 1.2.3-beta.1）"
}

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

<#
    在单个文件里替换版本字段。
    $Pattern 必须提供三个捕获组：1=前缀、2=旧版本、3=后缀。
#>
function Set-VersionInFile {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Pattern,
        [Parameter(Mandatory)][string]$Label
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "找不到文件：$Path"
    }

    $raw = [System.IO.File]::ReadAllText($Path)
    $match = [regex]::Match($raw, $Pattern)
    if (-not $match.Success) {
        throw "在 $Label 中未找到可替换的版本字段：$Path"
    }

    $current = $match.Groups[2].Value
    if ($current -eq $normalized) {
        Write-Host ("  = {0,-22} 已是 {1}" -f $Label, $normalized)
        return $false
    }

    $rebuilt = $match.Groups[1].Value + $normalized + $match.Groups[3].Value
    $updated = $raw.Substring(0, $match.Index) + $rebuilt + $raw.Substring($match.Index + $match.Length)
    [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)

    Write-Host ("  > {0,-22} {1} -> {2}" -f $Label, $current, $normalized)
    return $true
}

Write-Host "同步版本号到 $normalized"

# package.json 的顶层 "version"
Set-VersionInFile `
    -Path (Join-Path $RepoRoot 'package.json') `
    -Pattern '(?m)^(\s*"version"\s*:\s*")([^"]*)(")' `
    -Label 'package.json' | Out-Null

# Cargo.toml 的 [package] version（行首锚定，避免命中 `tauri = { version = "2" }` 这类内联字段）
Set-VersionInFile `
    -Path (Join-Path $RepoRoot 'src-tauri/Cargo.toml') `
    -Pattern '(?m)^(version\s*=\s*")([^"]*)(")' `
    -Label 'Cargo.toml' | Out-Null

# tauri.conf.json 的顶层 "version"
Set-VersionInFile `
    -Path (Join-Path $RepoRoot 'src-tauri/tauri.conf.json') `
    -Pattern '(?m)^(\s*"version"\s*:\s*")([^"]*)(")' `
    -Label 'tauri.conf.json' | Out-Null

Write-Host '版本号同步完成'
