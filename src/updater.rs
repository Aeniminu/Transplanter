use std::cmp::Ordering as CmpOrdering;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const RELEASE_API_URL: &str = "https://api.github.com/repos/Aeniminu/Transplanter/releases/latest";
const RELEASE_ASSET_NAME: &str = "Transplanter.exe";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub notes: String,
    pub html_url: String,
    pub asset_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateCheck {
    pub latest: ReleaseInfo,
    pub update_available: bool,
}

pub fn check_for_update() -> Result<UpdateCheck, String> {
    let release = fetch_latest_release()?;
    let update_available = is_newer_version(&release.version, env!("CARGO_PKG_VERSION"));
    Ok(UpdateCheck {
        latest: release,
        update_available,
    })
}

pub fn download_update(release: &ReleaseInfo, exe_dir: &Path) -> Result<PathBuf, String> {
    let new_path = exe_dir.join(format!("{RELEASE_ASSET_NAME}.new"));
    if new_path.exists() {
        fs::remove_file(&new_path).map_err(|err| {
            format!(
                "エラー: 古い更新ファイル `{}` を削除できません: {err}",
                new_path.display()
            )
        })?;
    }

    let script = r#"
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $args[0] -OutFile $args[1] -UseBasicParsing
"#;
    let output = powershell_command(script)
        .arg(&release.asset_url)
        .arg(&new_path)
        .output()
        .map_err(|err| format!("エラー: 更新ファイルのダウンロードを開始できません: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "エラー: 更新ファイルをダウンロードできません。\n{}",
            command_details(&output)
        ));
    }

    let metadata = fs::metadata(&new_path).map_err(|err| {
        format!(
            "エラー: ダウンロード済み更新ファイル `{}` を確認できません: {err}",
            new_path.display()
        )
    })?;
    if metadata.len() == 0 {
        return Err("エラー: ダウンロードした更新ファイルが空です".to_string());
    }

    Ok(new_path)
}

pub fn launch_update_script(new_path: &Path) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|err| format!("エラー: 現在の実行ファイルの場所を確認できません: {err}"))?;
    let script_path = exe_path.with_file_name("Transplanter-update.ps1");
    fs::write(&script_path, update_script()).map_err(|err| {
        format!(
            "エラー: 更新用スクリプト `{}` を作成できません: {err}",
            script_path.display()
        )
    })?;

    powershell_file(&script_path)
        .arg(process::id().to_string())
        .arg(&exe_path)
        .arg(new_path)
        .spawn()
        .map_err(|err| format!("エラー: 更新用スクリプトを起動できません: {err}"))?;

    Ok(())
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    compare_versions(latest, current) == CmpOrdering::Greater
}

fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let script = format!(
        r#"
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$headers = @{{ 'User-Agent' = 'Transplanter' }}
$release = Invoke-RestMethod -Headers $headers -Uri '{RELEASE_API_URL}'
$asset = $release.assets | Where-Object {{ $_.name -eq '{RELEASE_ASSET_NAME}' }} | Select-Object -First 1
if ($null -eq $asset) {{ throw 'Release asset {RELEASE_ASSET_NAME} was not found.' }}
$body = if ($null -eq $release.body) {{ '' }} else {{ $release.body }}
$body_b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($body))
Write-Output ('tag=' + $release.tag_name)
Write-Output ('html_url=' + $release.html_url)
Write-Output ('asset_url=' + $asset.browser_download_url)
Write-Output ('notes_b64=' + $body_b64)
"#
    );

    let output = powershell_command(&script)
        .output()
        .map_err(|err| format!("エラー: GitHub Release の確認を開始できません: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "エラー: GitHub Release を確認できません。\n{}",
            command_details(&output)
        ));
    }

    parse_release_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_release_output(output: &str) -> Result<ReleaseInfo, String> {
    let mut tag = String::new();
    let mut html_url = String::new();
    let mut asset_url = String::new();
    let mut notes_b64 = String::new();

    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "tag" => tag = value.trim().to_string(),
            "html_url" => html_url = value.trim().to_string(),
            "asset_url" => asset_url = value.trim().to_string(),
            "notes_b64" => notes_b64 = value.trim().to_string(),
            _ => {}
        }
    }

    if tag.is_empty() || asset_url.is_empty() {
        return Err("エラー: GitHub Release の情報が不足しています".to_string());
    }

    let notes = decode_base64_utf8(&notes_b64)?;
    let version = tag.trim_start_matches('v').to_string();
    Ok(ReleaseInfo {
        tag,
        version,
        notes,
        html_url,
        asset_url,
    })
}

fn compare_versions(left: &str, right: &str) -> CmpOrdering {
    let left = version_numbers(left);
    let right = version_numbers(right);
    let len = left.len().max(right.len());

    for index in 0..len {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            CmpOrdering::Equal => {}
            ordering => return ordering,
        }
    }

    CmpOrdering::Equal
}

fn version_numbers(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn decode_base64_utf8(value: &str) -> Result<String, String> {
    let bytes = decode_base64(value)?;
    String::from_utf8(bytes)
        .map_err(|err| format!("エラー: リリースノートをUTF-8として読めません: {err}"))
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for ch in value.chars().filter(|ch| !ch.is_ascii_whitespace()) {
        if ch == '=' {
            break;
        }
        let Some(value) = base64_value(ch) else {
            return Err("エラー: リリースノートのBase64を読めません".to_string());
        };
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

fn base64_value(ch: char) -> Option<u8> {
    match ch {
        'A'..='Z' => Some(ch as u8 - b'A'),
        'a'..='z' => Some(ch as u8 - b'a' + 26),
        '0'..='9' => Some(ch as u8 - b'0' + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

fn powershell_command(script: &str) -> Command {
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

fn powershell_file(script_path: &Path) -> Command {
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(script_path)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

fn command_details(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        stderr.trim().to_string()
    }
}

fn update_script() -> &'static str {
    r#"
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$TargetPath,
    [Parameter(Mandatory = $true)][string]$NewPath
)

$ErrorActionPreference = 'Stop'
$backupPath = "$TargetPath.old"

try {
    Wait-Process -Id $ProcessId -Timeout 30 -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500

    if (Test-Path -LiteralPath $backupPath) {
        Remove-Item -LiteralPath $backupPath -Force
    }

    Rename-Item -LiteralPath $TargetPath -NewName (Split-Path -Leaf $backupPath)
    Rename-Item -LiteralPath $NewPath -NewName (Split-Path -Leaf $TargetPath)

    Start-Process -FilePath $TargetPath
    Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
} catch {
    if (!(Test-Path -LiteralPath $TargetPath) -and (Test-Path -LiteralPath $backupPath)) {
        Rename-Item -LiteralPath $backupPath -NewName (Split-Path -Leaf $TargetPath) -Force
    }
    if (Test-Path -LiteralPath $TargetPath) {
        Start-Process -FilePath $TargetPath
    }
} finally {
    Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_detects_newer_release() {
        assert!(is_newer_version("v0.1.1", "0.1.0"));
        assert!(is_newer_version("0.1.10", "0.1.2"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.1"));
    }

    #[test]
    fn release_output_parses_base64_notes() {
        let release = parse_release_output(
            "tag=v0.1.1\nhtml_url=https://example.test/release\nasset_url=https://example.test/Transplanter.exe\nnotes_b64=44OG44K544OI\n",
        )
        .unwrap();

        assert_eq!(release.tag, "v0.1.1");
        assert_eq!(release.version, "0.1.1");
        assert_eq!(release.notes, "テスト");
        assert_eq!(release.asset_url, "https://example.test/Transplanter.exe");
    }
}
