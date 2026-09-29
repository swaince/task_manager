<#
.SYNOPSIS
    GitLab CI 在 Windows runner 上的构建入口：解析版本 -> 准备工具链 -> 编译 -> 打包。

.DESCRIPTION
    这个脚本是「一处编写、两处运行」的：CI 里直接调用，
    本地也能用 `pwsh scripts/ci-build-windows.ps1` 完整复跑一遍流水线的构建阶段。

    版本来源优先级：
      1. $env:CI_COMMIT_TAG（形如 v1.0.0）—— tag 流水线
      2. $env:BUILD_VERSION      —— 手动指定
      3. src-tauri/tauri.conf.json —— 普通分支构建

    产出的 `artifacts/build.env` 会被 GitLab 的 artifact:reports:dotenv 收集，
    供后续 release 作业读取（安装包名、免安装包名、包仓库版本号等）。

.PARAMETER SkipInstall
    跳过 `pnpm install`（本地已装好依赖时复用，省时间）。

.PARAMETER SkipToolchain
    不尝试安装缺失的 Rust / Node（用于工具链已就绪的自建 runner，或只想做检查）。

.EXAMPLE
    pwsh scripts/ci-build-windows.ps1

.EXAMPLE
    pwsh scripts/ci-build-windows.ps1 -SkipInstall -SkipToolchain
#>
[CmdletBinding()]
param(
    [switch]$SkipInstall,
    [switch]$SkipToolchain,
    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

# 注意：$PSScriptRoot 在 `-File` 调用形式的参数默认值里可能为空，
# 因此不在 param 块里求值，改用 $MyInvocation 推导脚本目录。
$ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Definition
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent $ScriptDirectory
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
Set-Location -LiteralPath $RepoRoot

function Write-Step([string]$Message) {
    Write-Host ''
    Write-Host "=== $Message" -ForegroundColor Cyan
}

function Test-Command([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Add-ToPath([string]$Directory) {
    if ((Test-Path -LiteralPath $Directory) -and ($env:Path -notlike "*$Directory*")) {
        $env:Path = "$Directory;$env:Path"
    }
}

function Update-PathFromRegistry {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = (@($machine, $user) | Where-Object { $_ }) -join ';'
}

# ---------------------------------------------------------------------------
# 1. 决定版本号
# ---------------------------------------------------------------------------
function Resolve-AppVersion {
    if ($env:CI_COMMIT_TAG -and $env:CI_COMMIT_TAG -match '^v?\d+\.\d+\.\d+') {
        return ($env:CI_COMMIT_TAG -replace '^[vV]', '')
    }
    if ($env:BUILD_VERSION) {
        return ($env:BUILD_VERSION -replace '^[vV]', '')
    }
    $config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
    return [string]$config.version
}

Write-Step '解析版本号'
$appVersion = Resolve-AppVersion
# 包仓库里的版本必须唯一（重复上传同名文件会被拒绝），
# 因此非 tag 构建追加流水线号；安装包本身仍用干净的 semver。
$packageVersion = $appVersion
if (-not $env:CI_COMMIT_TAG -and $env:CI_PIPELINE_IID) {
    $packageVersion = "$appVersion-latest.$($env:CI_PIPELINE_IID)"
}
Write-Host "应用版本   : $appVersion"
Write-Host "包仓库版本 : $packageVersion"
if ($env:CI_COMMIT_TAG) { Write-Host "触发来源   : tag $($env:CI_COMMIT_TAG)" }
elseif ($env:CI_COMMIT_BRANCH) { Write-Host "触发来源   : branch $($env:CI_COMMIT_BRANCH)" }
else { Write-Host '触发来源   : 本地运行' }

# ---------------------------------------------------------------------------
# 2. 工具链
# ---------------------------------------------------------------------------
Write-Step '检查工具链'

if (-not (Test-Command 'node')) {
    if ($SkipToolchain) { throw '未安装 Node.js，且指定了 -SkipToolchain' }
    Write-Host '未检测到 Node.js，开始安装…'
    if (Test-Command 'choco') {
        choco install nodejs-lts -y --no-progress
        Update-PathFromRegistry
    }
    else {
        # 兜底：直接取官方 zip，免管理员权限
        $index = Invoke-RestMethod -Uri 'https://nodejs.org/dist/index.json' -UseBasicParsing
        $lts = $index | Where-Object { $_.lts } | Select-Object -First 1
        $zipName = "node-$($lts.version)-win-x64.zip"
        $zipPath = Join-Path $env:TEMP $zipName
        $extractRoot = Join-Path $RepoRoot '.toolchain'
        Invoke-WebRequest -Uri "https://nodejs.org/dist/$($lts.version)/$zipName" -OutFile $zipPath -UseBasicParsing
        New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null
        Expand-Archive -LiteralPath $zipPath -DestinationPath $extractRoot -Force
        Add-ToPath (Join-Path $extractRoot "node-$($lts.version)-win-x64")
    }
}
Write-Host "node  : $(node --version)"

if (-not (Test-Command 'cargo')) {
    if ($SkipToolchain) { throw '未安装 Rust，且指定了 -SkipToolchain' }
    Write-Host '未检测到 Rust，开始通过 rustup 安装…'
    $rustup = Join-Path $env:TEMP 'rustup-init.exe'
    Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' `
        -OutFile $rustup -UseBasicParsing
    & $rustup -y --profile minimal --default-toolchain stable --no-modify-path
    if ($LASTEXITCODE -ne 0) { throw "rustup 安装失败（exit $LASTEXITCODE）" }
    if ($env:CARGO_HOME) { Add-ToPath (Join-Path $env:CARGO_HOME 'bin') }
    Add-ToPath (Join-Path $env:USERPROFILE '.cargo\bin')
}
Write-Host "rustc : $(rustc --version)"
Write-Host "cargo : $(cargo --version)"

if (-not (Test-Command 'pnpm')) {
    Write-Host '未检测到 pnpm，尝试通过 corepack 启用…'
    if (Test-Command 'corepack') {
        corepack enable
    }
    if (-not (Test-Command 'pnpm')) {
        Write-Host 'corepack 不可用，改用 npm 全局安装 pnpm…'
        npm install -g pnpm
        Update-PathFromRegistry
        Add-ToPath (Join-Path $env:APPDATA 'npm')
    }
}
Write-Host "pnpm  : $(pnpm --version)"

# ---------------------------------------------------------------------------
# 3. 同步版本号
# ---------------------------------------------------------------------------
Write-Step "同步版本号到 $appVersion"
& (Join-Path $ScriptDirectory 'set-version.ps1') -Version $appVersion -RepoRoot $RepoRoot

# ---------------------------------------------------------------------------
# 4. 依赖与编译
# ---------------------------------------------------------------------------
if (-not $SkipInstall) {
    Write-Step '安装前端依赖'
    pnpm install --frozen-lockfile
    if ($LASTEXITCODE -ne 0) { throw "pnpm install 失败（exit $LASTEXITCODE）" }
}
else {
    Write-Step '跳过 pnpm install（-SkipInstall）'
}

Write-Step '编译（前端 + Rust release + NSIS 安装包）'
# 直接调用 @tauri-apps/cli 的 Node 入口，而不是 `pnpm tauri build`：
# pnpm 的安装方式（corepack / 全局安装 / 各类包装 shim）在透传脚本参数时行为并不一致，
# 直接跑入口文件可以完全绕开这一层，runner 上更稳。找不到入口时退回 pnpm。
$tauriEntry = Join-Path $RepoRoot 'node_modules/@tauri-apps/cli/tauri.js'
if (Test-Path -LiteralPath $tauriEntry) {
    node $tauriEntry build --bundles nsis
}
else {
    Write-Host '未找到 @tauri-apps/cli 入口，回退到 pnpm tauri build'
    pnpm tauri build --bundles nsis
}
if ($LASTEXITCODE -ne 0) { throw "Tauri 构建失败（exit $LASTEXITCODE）" }

# ---------------------------------------------------------------------------
# 5. 打包交付物
# ---------------------------------------------------------------------------
Write-Step '生成安装版与免安装版'
& (Join-Path $ScriptDirectory 'package-windows.ps1') `
    -Version $appVersion `
    -OutDir (Join-Path $RepoRoot 'artifacts') `
    -RepoRoot $RepoRoot

# build.env 里的 PACKAGE_VERSION 需要按流水线唯一化，这里覆盖一次
$envFile = Join-Path $RepoRoot 'artifacts/build.env'
$lines = Get-Content -LiteralPath $envFile | Where-Object { $_ -notmatch '^PACKAGE_VERSION=' }
$lines = @("PACKAGE_VERSION=$packageVersion") + $lines
[System.IO.File]::WriteAllLines($envFile, $lines, (New-Object System.Text.UTF8Encoding($false)))

# 把变量写进 CI 作业环境，方便同作业后续步骤直接使用
if ($env:GITLAB_CI) {
    foreach ($line in $lines) {
        $pair = $line.Split('=', 2)
        if ($pair.Count -eq 2) {
            Set-Item -Path "Env:$($pair[0])" -Value $pair[1]
        }
    }
}

Write-Step '完成'
Write-Host "应用版本：$appVersion"
Write-Host "包仓库版本：$packageVersion"
