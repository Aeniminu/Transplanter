use std::cmp::Ordering as CmpOrdering;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{self, Command};
use std::ptr::null_mut;

use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

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

pub fn launch_update_script(release: &ReleaseInfo) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|err| format!("エラー: 現在の実行ファイルの場所を確認できません: {err}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "エラー: 現在の実行ファイルのフォルダを確認できません".to_string())?;
    let new_path = exe_dir.join(format!("{RELEASE_ASSET_NAME}.new"));
    let script_path = exe_path.with_file_name("Transplanter-update.ps1");
    fs::write(&script_path, update_script()).map_err(|err| {
        format!(
            "エラー: 更新用スクリプト `{}` を作成できません: {err}",
            script_path.display()
        )
    })?;

    launch_powershell_file(
        &script_path,
        &[
            process::id().to_string(),
            exe_path.display().to_string(),
            new_path.display().to_string(),
            release.asset_url.clone(),
        ],
    )?;

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

fn launch_powershell_file(script_path: &Path, args: &[String]) -> Result<(), String> {
    let operation = wide("open");
    let executable = wide("powershell.exe");
    let parameters = wide(&powershell_file_parameters(script_path, args));
    let directory = script_path
        .parent()
        .and_then(Path::to_str)
        .map(wide)
        .unwrap_or_else(|| wide(""));

    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            operation.as_ptr(),
            executable.as_ptr(),
            parameters.as_ptr(),
            directory.as_ptr(),
            SW_HIDE,
        )
    } as isize;

    if result <= 32 {
        return Err(format!(
            "エラー: 更新用スクリプトを起動できません: ShellExecuteW エラーコード {result}"
        ));
    }

    Ok(())
}

fn powershell_file_parameters(script_path: &Path, args: &[String]) -> String {
    let mut parameters = vec![
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-WindowStyle".to_string(),
        "Hidden".to_string(),
        "-File".to_string(),
        quote_windows_arg(&script_path.display().to_string()),
    ];
    parameters.extend(args.iter().map(|arg| quote_windows_arg(arg)));
    parameters.join(" ")
}

fn quote_windows_arg(value: &str) -> String {
    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;

    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                quoted.push(ch);
                backslashes = 0;
            }
        }
    }

    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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
    [Parameter(Mandatory = $true)][string]$NewPath,
    [Parameter(Mandatory = $true)][string]$AssetUrl
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$backupPath = "$TargetPath.old"
$logPath = "$TargetPath.update.log"

try {
    Wait-Process -Id $ProcessId -Timeout 30 -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500

    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    if (Test-Path -LiteralPath $NewPath) {
        Remove-Item -LiteralPath $NewPath -Force
    }
    Invoke-WebRequest -Uri $AssetUrl -OutFile $NewPath -UseBasicParsing
    if (!(Test-Path -LiteralPath $NewPath) -or ((Get-Item -LiteralPath $NewPath).Length -le 0)) {
        throw 'Downloaded update file is empty.'
    }

    if (Test-Path -LiteralPath $backupPath) {
        Remove-Item -LiteralPath $backupPath -Force
    }

    Rename-Item -LiteralPath $TargetPath -NewName (Split-Path -Leaf $backupPath)
    Rename-Item -LiteralPath $NewPath -NewName (Split-Path -Leaf $TargetPath)

    Start-Process -FilePath $TargetPath
    Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
} catch {
    $_ | Out-String | Set-Content -LiteralPath $logPath -Encoding UTF8
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

    #[test]
    fn update_script_downloads_after_current_process_exits() {
        let script = update_script();

        assert!(script.contains("[string]$AssetUrl"));
        assert!(script.contains("Wait-Process -Id $ProcessId"));
        assert!(script.contains("Invoke-WebRequest -Uri $AssetUrl"));
        assert!(script.contains("Rename-Item -LiteralPath $TargetPath"));
        assert!(script.contains("Start-Process -FilePath $TargetPath"));
    }

    #[test]
    fn powershell_file_parameters_quote_update_arguments() {
        let parameters = powershell_file_parameters(
            Path::new(r"C:\Users\Player\Transplanter App\Transplanter-update.ps1"),
            &[
                "1234".to_string(),
                r"C:\Users\Player\Transplanter App\Transplanter.exe".to_string(),
                r"C:\Users\Player\Transplanter App\Transplanter.exe.new".to_string(),
                "https://example.test/Transplanter.exe".to_string(),
            ],
        );

        assert!(parameters.contains("-WindowStyle Hidden"));
        assert!(
            parameters
                .contains(r#"-File "C:\Users\Player\Transplanter App\Transplanter-update.ps1""#)
        );
        assert!(parameters.contains(r#""C:\Users\Player\Transplanter App\Transplanter.exe""#));
        assert!(parameters.contains(r#""https://example.test/Transplanter.exe""#));
    }

    #[test]
    fn quote_windows_arg_escapes_quotes_and_trailing_backslashes() {
        assert_eq!(
            quote_windows_arg(r#"C:\Path With "Quote"\tail\"#),
            r#""C:\Path With \"Quote\"\tail\\""#
        );
    }
}
