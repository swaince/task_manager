<#
.SYNOPSIS
    把 artifacts/ 下的产物上传到 GitLab 通用软件包仓库（Generic Package Registry）。

.DESCRIPTION
    为什么不用流水线作业制品（job artifacts）当分发渠道：作业制品默认会过期，
    而 release 上的下载链接一旦过期就是个死链。通用软件包仓库里的文件是持久的，
    并且有稳定的 URL：

      $CI_API_V4_URL/projects/$CI_PROJECT_ID/packages/generic/<package>/<version>/<file>

    需要的环境变量（GitLab CI 自动提供）：
      CI_API_V4_URL   形如 https://gitlab.example.com/api/v4
      CI_PROJECT_ID
      CI_JOB_TOKEN
      PACKAGE_VERSION 由 artifacts/build.env 提供

.PARAMETER ArtifactDir
    待上传目录，默认 <RepoRoot>/artifacts。

.PARAMETER PackageName
    包仓库里的包名，需与 .gitlab-ci.yml 中的 PACKAGE_NAME 一致。

.EXAMPLE
    pwsh scripts/ci-publish-packages.ps1
#>
[CmdletBinding()]
param(
    [string]$ArtifactDir,
    [string]$PackageName = 'process-manager',
    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

# $PSScriptRoot 在 `-File` 调用形式的参数默认值里可能为空，放到脚本体里求值。
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
}

if ([string]::IsNullOrWhiteSpace($ArtifactDir)) {
    $ArtifactDir = Join-Path $RepoRoot 'artifacts'
}

foreach ($required in @('CI_API_V4_URL', 'CI_PROJECT_ID', 'CI_JOB_TOKEN')) {
    if (-not (Get-Item "Env:$required" -ErrorAction SilentlyContinue)) {
        throw "缺少环境变量 $required —— 该脚本只能在 GitLab CI 中运行"
    }
}

if (-not $env:PACKAGE_VERSION) {
    $envFile = Join-Path $ArtifactDir 'build.env'
    if (Test-Path -LiteralPath $envFile) {
        foreach ($line in Get-Content -LiteralPath $envFile) {
            $pair = $line.Split('=', 2)
            if ($pair.Count -eq 2 -and $pair[0] -eq 'PACKAGE_VERSION') {
                $env:PACKAGE_VERSION = $pair[1]
            }
        }
    }
}
if (-not $env:PACKAGE_VERSION) {
    throw '无法确定 PACKAGE_VERSION（既没有环境变量，也没有 artifacts/build.env）'
}

# PowerShell 5.1 默认可能不启用 TLS 1.2
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$baseUrl = "$($env:CI_API_V4_URL)/projects/$($env:CI_PROJECT_ID)/packages/generic/$PackageName/$($env:PACKAGE_VERSION)"
$uploadFiles = Get-ChildItem -LiteralPath $ArtifactDir -File |
    Where-Object { $_.Name -ne 'build.env' }

Write-Host "上传到通用软件包仓库：$PackageName/$($env:PACKAGE_VERSION)"
Write-Host "目标前缀：$baseUrl"
Write-Host ''

$failed = @()
foreach ($file in $uploadFiles) {
    $uri = "$baseUrl/$($file.Name)"
    try {
        Invoke-RestMethod -Method Put -Uri $uri -InFile $file.FullName `
            -Headers @{ 'JOB-TOKEN' = $env:CI_JOB_TOKEN } -TimeoutSec 600 | Out-Null
        Write-Host ("  [ok]   {0}" -f $file.Name)
    }
    catch {
        $status = $null
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        if ($status -eq 400 -or $status -eq 403) {
            # GitLab 不允许覆盖已存在的同名包文件；重跑同一条流水线时会走到这里。
            # 内容与已上传的应一致，告警而不中断发布。
            Write-Warning "  [skip] $($file.Name) 已存在（HTTP $status），沿用已有文件"
        }
        else {
            Write-Warning "  [fail] $($file.Name) —— $($_.Exception.Message)"
            $failed += $file.Name
        }
    }
}

if ($failed.Count -gt 0) {
    throw "以下文件上传失败：$($failed -join ', ')"
}

Write-Host ''
Write-Host '下载地址（可直接写进 release 资产链接）：'
foreach ($file in $uploadFiles) {
    Write-Host "  $baseUrl/$($file.Name)"
}
