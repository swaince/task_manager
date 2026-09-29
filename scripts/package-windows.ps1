<#
.SYNOPSIS
    把 `tauri build` 的产物整理成「安装版 + 免安装版」两件交付物。

.DESCRIPTION
    输入（由 `pnpm tauri build --bundles nsis` 生成）：
      src-tauri/target/release/<mainBinaryName>.exe        可执行文件本体
      src-tauri/target/release/bundle/nsis/*-setup.exe     NSIS 安装程序

    输出（默认写入 <RepoRoot>/artifacts）：
      <Slug>_<version>_<arch>-setup.exe       安装版
      <Slug>_<version>_<arch>_portable.zip    免安装版（解压即用）
      SHA256SUMS.txt                          两个产物的校验和
      build.env                               供 GitLab CI 的 artifacts:reports:dotenv 使用

    产物统一改成 ASCII 文件名：productName 是中文，而安装包名里带中文会在
    URL、CI 制品路径与各平台上带来不必要的编码麻烦。

.PARAMETER Version
    交付版本；省略时从 tauri.conf.json 读取。

.PARAMETER OutDir
    输出目录，默认 <RepoRoot>/artifacts。

.PARAMETER Slug
    产物文件名前缀，默认 ProcessManager。

.EXAMPLE
    pwsh scripts/package-windows.ps1 -Version 1.0.0
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$OutDir,
    [string]$Slug = 'ProcessManager',
    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'

# $PSScriptRoot 在 `-File` 调用形式的参数默认值里可能为空，放到脚本体里求值。
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path

$targetRelease = Join-Path $RepoRoot 'src-tauri/target/release'
$configPath = Join-Path $RepoRoot 'src-tauri/tauri.conf.json'

if (-not (Test-Path -LiteralPath $configPath)) {
    throw "找不到 Tauri 配置：$configPath"
}
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json

# --- 版本：未显式传入时取仓库里的版本 -----------------------------------
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = [string]$config.version
    Write-Host "未指定 -Version，使用 tauri.conf.json 中的 $Version"
}
$Version = $Version.Trim()
if ($Version.StartsWith('v') -or $Version.StartsWith('V')) {
    $Version = $Version.Substring(1)
}
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw '无法确定版本号，请显式传入 -Version'
}

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $RepoRoot 'artifacts'
}

# --- 架构 ---------------------------------------------------------------
$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'ARM64' { 'arm64' }
    'x86' { 'x86' }
    default { 'x64' }
}

# --- 定位待打包的文件 ----------------------------------------------------
$binaryName = 'process-manager'
if ($config.PSObject.Properties.Name -contains 'mainBinaryName' -and $config.mainBinaryName) {
    $binaryName = [string]$config.mainBinaryName
}
$binaryPath = Join-Path $targetRelease "$binaryName.exe"
if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "找不到可执行文件：$binaryPath`n请先执行 pnpm tauri build --bundles nsis"
}

$nsisDir = Join-Path $targetRelease 'bundle/nsis'
if (-not (Test-Path -LiteralPath $nsisDir)) {
    throw "找不到 NSIS 产物目录：$nsisDir`n请先执行 pnpm tauri build --bundles nsis"
}
$installerSource = Get-ChildItem -LiteralPath $nsisDir -Filter '*.exe' -File |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $installerSource) {
    throw "在 $nsisDir 中没有找到任何 .exe 安装包"
}

# --- 输出 ---------------------------------------------------------------
if (Test-Path -LiteralPath $OutDir) {
    Remove-Item -LiteralPath $OutDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$installerName = "${Slug}_${Version}_${arch}-setup.exe"
$portableName = "${Slug}_${Version}_${arch}_portable.zip"

$installerOut = Join-Path $OutDir $installerName
Copy-Item -LiteralPath $installerSource.FullName -Destination $installerOut -Force
Write-Host "安装版   $(Split-Path -Leaf $installerSource.FullName)  ->  $installerName"

# 免安装版：一个目录 + zip，内含 exe（改成 ASCII 名）与使用说明
$staging = Join-Path $OutDir 'portable'
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $staging "$Slug.exe") -Force

$usage = @"
$Slug $Version — 免安装版（Windows $arch）
================================================

直接双击 $Slug.exe 即可运行，无需安装、不写注册表、不留卸载项。
想「卸载」直接删掉整个文件夹即可。

运行要求
  * Windows 10 1809 / Windows 11 及以上
  * 需要 WebView2 运行时（Win11 与较新的 Win10 已内置；
    若提示缺失，请安装 https://developer.microsoft.com/microsoft-edge/webview2/ 的 Evergreen Runtime）

建议以管理员身份运行（右键 → 以管理员身份运行）
  普通权限下，系统进程与其它用户会话下的进程读不到命令行 / 环境变量，
  也无法结束；这类进程在列表里会标记为「受保护」。

首次运行可能被 SmartScreen 拦截：选择「更多信息」→「仍要运行」。
"@
[System.IO.File]::WriteAllText((Join-Path $staging '使用说明.txt'), $usage, (New-Object System.Text.UTF8Encoding($true)))

Compress-Archive -Path (Join-Path $staging '*.exe'), (Join-Path $staging '*.txt') `
    -DestinationPath (Join-Path $OutDir $portableName) -CompressionLevel Optimal -Force
Remove-Item -LiteralPath $staging -Recurse -Force
Write-Host "免安装版 $binaryName.exe  ->  $portableName"

# --- 校验和 -------------------------------------------------------------
$sums = foreach ($name in @($installerName, $portableName)) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $OutDir $name) -Algorithm SHA256).Hash.ToLower()
    "$hash  $name"
}
[System.IO.File]::WriteAllLines(
    (Join-Path $OutDir 'SHA256SUMS.txt'),
    $sums,
    (New-Object System.Text.UTF8Encoding($false))
)

# --- 供 CI 消费的 dotenv -------------------------------------------------
$env_lines = @(
    "PACKAGE_VERSION=$Version",
    "PRODUCT_SLUG=$Slug",
    "ARTIFACT_ARCH=$arch",
    "INSTALLER_FILE=$installerName",
    "PORTABLE_FILE=$portableName",
    "CHECKSUMS_FILE=SHA256SUMS.txt"
)
[System.IO.File]::WriteAllLines(
    (Join-Path $OutDir 'build.env'),
    $env_lines,
    (New-Object System.Text.UTF8Encoding($false))
)

Write-Host ''
Write-Host "产物目录：$OutDir"
Get-ChildItem -LiteralPath $OutDir -File | ForEach-Object {
    Write-Host ("  {0,-48} {1,10:N0} B" -f $_.Name, $_.Length)
}
