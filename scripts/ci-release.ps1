<#
.SYNOPSIS
    创建或更新 GitLab Release，并把「安装版 / 免安装版」挂成 release 资产链接。

.DESCRIPTION
    两种用法：
      * tag 流水线  ——  scripts/ci-release.ps1 -Tag v1.0.0
      * main 流水线 ——  scripts/ci-release.ps1 -Tag latest -Name "Latest（main 滚动构建）"

    对 main 的滚动 release，如果提供了 RELEASE_TOKEN（带 write_repository 权限的
    项目访问令牌），会先把 `latest` tag 强制移动到当前提交，保证 release 上的
    "Source code" 与产物同源；没有该令牌时只更新 release 内容，tag 保持首次创建的位置。

    需要的环境变量（GitLab CI 自动提供）：
      CI_API_V4_URL / CI_PROJECT_ID / CI_JOB_TOKEN / CI_COMMIT_SHA
      CI_PROJECT_URL / CI_COMMIT_REF_NAME
    以及来自 artifacts/build.env 的：
      PACKAGE_VERSION / INSTALLER_FILE / PORTABLE_FILE / CHECKSUMS_FILE

.PARAMETER Tag
    release 关联的 tag 名。

.PARAMETER Name
    release 标题，默认与 Tag 相同。

.EXAMPLE
    pwsh scripts/ci-release.ps1 -Tag v1.0.0
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Tag,
    [Parameter(Position = 1)][string]$Name,
    [string]$PackageName = 'process-manager',
    [string]$ArtifactDir,
    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

# $PSScriptRoot 在 `-File` 调用形式的参数默认值里可能为空，放到脚本体里求值。
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
}

if ([string]::IsNullOrWhiteSpace($Name)) { $Name = $Tag }
if ([string]::IsNullOrWhiteSpace($ArtifactDir)) { $ArtifactDir = Join-Path $RepoRoot 'artifacts' }

foreach ($required in @('CI_API_V4_URL', 'CI_PROJECT_ID', 'CI_JOB_TOKEN', 'CI_COMMIT_SHA')) {
    if (-not (Get-Item "Env:$required" -ErrorAction SilentlyContinue)) {
        throw "缺少环境变量 $required —— 该脚本只能在 GitLab CI 中运行"
    }
}

# --- 读取构建阶段写下的产物信息 -----------------------------------------
$envFile = Join-Path $ArtifactDir 'build.env'
if (-not (Test-Path -LiteralPath $envFile)) {
    throw "找不到 $envFile —— release 作业需要通过 needs:artifacts 拿到构建产物"
}
foreach ($line in Get-Content -LiteralPath $envFile) {
    if ($line -match '^([A-Z_]+)=(.*)$') {
        Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
    }
}
foreach ($required in @('PACKAGE_VERSION', 'INSTALLER_FILE', 'PORTABLE_FILE')) {
    if (-not (Get-Item "Env:$required" -ErrorAction SilentlyContinue)) {
        throw "build.env 中缺少 $required"
    }
}
if (-not $env:CHECKSUMS_FILE) { $env:CHECKSUMS_FILE = 'SHA256SUMS.txt' }

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$projectApi = "$($env:CI_API_V4_URL)/projects/$($env:CI_PROJECT_ID)"
$registryBase = "$projectApi/packages/generic/$PackageName/$($env:PACKAGE_VERSION)"

# --- 调用 GitLab API 的小helper：把 HTTP 错误码变成返回值而不是异常 --------
function Invoke-GitLabApi {
    param(
        [Parameter(Mandatory)][string]$Method,
        [Parameter(Mandatory)][string]$Uri,
        $Body
    )
    $headers = @{ 'JOB-TOKEN' = $env:CI_JOB_TOKEN }
    try {
        if ($null -ne $Body) {
            $json = $Body | ConvertTo-Json -Depth 10
            $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
            $content = Invoke-RestMethod -Method $Method -Uri $Uri -Headers $headers `
                -ContentType 'application/json; charset=utf-8' -Body $bytes -TimeoutSec 120
        }
        else {
            $content = Invoke-RestMethod -Method $Method -Uri $Uri -Headers $headers -TimeoutSec 120
        }
        return @{ Status = 200; Content = $content; Error = $null }
    }
    catch {
        $status = 0
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        return @{ Status = $status; Content = $null; Error = $_.Exception.Message }
    }
}

# --- main 的滚动 release：把 latest tag 移到当前提交 ----------------------
if ($Tag -eq 'latest' -and $env:RELEASE_TOKEN) {
    Write-Host '检测到 RELEASE_TOKEN，尝试把 latest tag 移动到当前提交…'
    git tag -f latest $env:CI_COMMIT_SHA | Out-Host
    $remote = "https://oauth2:$($env:RELEASE_TOKEN)@$($env:CI_SERVER_HOST)/$($env:CI_PROJECT_PATH).git"
    git push -f $remote refs/tags/latest 2>&1 | Out-Host
    if ($LASTEXITCODE -ne 0) {
        Write-Warning 'latest tag 移动失败（令牌权限不足？），继续更新 release 内容'
    }
}
elseif ($Tag -eq 'latest') {
    Write-Host '未配置 RELEASE_TOKEN，latest tag 保持原位置，仅更新 release 内容与资产链接'
}

# --- 组装 release 内容 ---------------------------------------------------
$shortSha = $env:CI_COMMIT_SHA.Substring(0, 8)
$description = @"
## Windows 构建

| 项目 | 值 |
| --- | --- |
| 应用版本 | ``$env:PACKAGE_VERSION`` |
| 提交 | ``$shortSha`` |
| 分支 / 标签 | ``$env:CI_COMMIT_REF_NAME`` |

### 下载

| 版本 | 文件 | 说明 |
| --- | --- | --- |
| **安装版** | ``$($env:INSTALLER_FILE)`` | 双击安装，写入开始菜单与卸载项，可选「仅当前用户 / 所有用户」 |
| **免安装版** | ``$($env:PORTABLE_FILE)`` | 解压即用，不写注册表，删除目录即完成卸载 |

> **运行要求**：Windows 10 1809+ / Windows 11，需要 WebView2 运行时（较新系统已内置）。
> **建议以管理员身份运行** —— 普通权限下受保护进程的命令行、环境变量读不到，也无法结束。

校验和见 ``$($env:CHECKSUMS_FILE)``。
"@

$releaseBody = @{
    name        = $Name
    tag_name    = $Tag
    ref         = $env:CI_COMMIT_SHA
    description = $description
    assets      = @{
        links = @(
            @{ name = 'Windows 安装版 (.exe)'; url = "$registryBase/$($env:INSTALLER_FILE)"; link_type = 'package' }
            @{ name = 'Windows 免安装版 (.zip)'; url = "$registryBase/$($env:PORTABLE_FILE)"; link_type = 'package' }
            @{ name = 'SHA256 校验和'; url = "$registryBase/$($env:CHECKSUMS_FILE)"; link_type = 'other' }
        )
    }
}

Write-Host "创建/更新 release：$Tag（标题：$Name）"

# 先删掉同名旧 release，让「创建」这条路永远走得通（滚动 latest 必需）
$deleted = Invoke-GitLabApi -Method Delete -Uri "$projectApi/releases/$Tag"
if ($deleted.Status -eq 200) { Write-Host '  已清理同名的旧 release' }

$created = Invoke-GitLabApi -Method Post -Uri "$projectApi/releases" -Body $releaseBody
switch ($created.Status) {
    200 { Write-Host '  [ok] release 已创建' }
    409 {
        Write-Host '  release 已存在且未能删除，改为更新…'
        $updated = Invoke-GitLabApi -Method Put -Uri "$projectApi/releases/$Tag" -Body $releaseBody
        if ($updated.Status -ne 200) {
            throw "更新 release 失败：HTTP $($updated.Status) $($updated.Error)"
        }
        Write-Host '  [ok] release 已更新'
    }
    default {
        throw "创建 release 失败：HTTP $($created.Status) $($created.Error)"
    }
}

Write-Host ''
Write-Host "release 页面：$($env:CI_PROJECT_URL)/-/releases/$Tag"
Write-Host "资产地址前缀：$registryBase"
