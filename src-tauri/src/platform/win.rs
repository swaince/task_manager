//! Windows 平台原语实现。
//!
//! 覆盖四类只有 Windows 才有的能力：
//! 1. **权限判定**：通过进程令牌判断本程序是否已提权；
//! 2. **窗口枚举**：`EnumWindows` → 进程的可视窗口标题（任务管理器风格的「应用」视图）；
//! 3. **图标抽取**：`SHGetFileInfoW` + `GetDIBits` 把 `HICON` 转成 PNG DataURL；
//! 4. **进程可读性**：`OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` 失败即视为受保护进程。

use crate::error::{ProcError, ProcResult};
use base64::Engine as _;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::{Mutex, OnceLock};

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessHandleCount, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::{
    SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, HICON,
    ICONINFO, SW_RESTORE,
};

/// Windows 错误码 `ERROR_ACCESS_DENIED`。
const ERROR_ACCESS_DENIED_CODE: i32 = 5;

pub fn platform_id() -> &'static str {
    "windows"
}

/// 当前进程是否以管理员（提升）权限运行。
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if windows::Win32::System::Threading::OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// 是否能够以 `PROCESS_QUERY_LIMITED_INFORMATION` 打开目标进程。
///
/// 打不开通常意味着该进程受保护或以更高完整性级别运行（例如系统进程、
/// 其他用户会话下的进程、带反作弊/杀软保护的进程），此时我们无法读取
/// 其命令行、工作目录与环境变量。
pub fn is_process_accessible(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(err) => {
                let code = err.code().0 as u32;
                // `HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)` 或原始 Win32 码都算「拒绝访问」。
                let hresult_denied = 0x8007_0000 | ERROR_ACCESS_DENIED_CODE as u32;
                code != hresult_denied && code != ERROR_ACCESS_DENIED_CODE as u32
            }
        }
    }
}

/// 目标进程的内核句柄数。
pub fn handle_count(pid: u32) -> Option<u32> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut count = 0u32;
        let ok = GetProcessHandleCount(handle, &mut count).is_ok();
        let _ = CloseHandle(handle);
        ok.then_some(count)
    }
}

/// 枚举所有可见且有标题的顶层窗口，按 pid 归组。
pub fn window_titles() -> HashMap<u32, Vec<String>> {
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    unsafe {
        let ptr: *mut HashMap<u32, Vec<String>> = &mut map;
        let _ = EnumWindows(Some(enum_window_proc), LPARAM(ptr as isize));
    }
    map
}

unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return TRUE;
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let copied = GetWindowTextW(hwnd, &mut buf);
    if copied <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buf[..copied as usize]);
    let title = title.trim().to_string();
    if title.is_empty() {
        return TRUE;
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return TRUE;
    }
    let map = &mut *(lparam.0 as *mut HashMap<u32, Vec<String>>);
    let entry = map.entry(pid).or_default();
    if entry.len() < 8 && !entry.contains(&title) {
        entry.push(title);
    }
    TRUE
}

/// 把目标进程的第一个可见窗口拉到前台。成功返回 true。
pub fn focus_window(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    TARGET_PID.with(|c| c.set(pid));
    let mut found: Option<HWND> = None;
    unsafe {
        let ptr: *mut Option<HWND> = &mut found;
        let _ = EnumWindows(Some(find_window_proc), LPARAM(ptr as isize));
    }
    TARGET_PID.with(|c| c.set(0));
    let Some(hwnd) = found else { return false };
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd).as_bool()
    }
}

unsafe extern "system" fn find_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target = &mut *(lparam.0 as *mut Option<HWND>);
    if target.is_some() || !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    if GetWindowTextLengthW(hwnd) <= 0 {
        return TRUE;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == current_target_pid() {
        *target = Some(hwnd);
        return BOOL(0); // 提前终止枚举
    }
    TRUE
}

/// `find_window_proc` 需要知道目标 pid；通过线程本地变量传递，
/// 避免在 `LPARAM` 里同时塞两个指针。
fn current_target_pid() -> u32 {
    TARGET_PID.with(|c| c.get())
}

thread_local! {
    static TARGET_PID: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// 在资源管理器中定位文件（选中该项）。
pub fn reveal_in_file_manager(path: &str) -> ProcResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(ProcError::Other("路径为空".into()));
    }
    let mut command = std::process::Command::new("explorer.exe");
    if std::path::Path::new(trimmed).exists() {
        command.arg(format!("/select,{trimmed}"));
    } else if let Some(parent) = std::path::Path::new(trimmed).parent() {
        command.arg(parent);
    } else {
        command.arg(trimmed);
    }
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| ProcError::Other(format!("无法启动资源管理器：{e}")))
}

/// 抽取可执行文件图标并缓存为 PNG DataURL。
///
/// 提取过程用一把全局锁串行化：`SHGetFileInfoW` 会访问 Shell 的图标缓存，
/// 多线程并发进入时偶发返回空图标（`GetDIBits` 拿不到像素）。
/// Tauri 的命令处理器本身就是多线程的，因此这不是"只有测试才会遇到"的问题。
/// 由于结果按 exe 路径缓存，同一路径最多只走一次临界区，代价可以忽略。
pub fn icon_data_url(exe: &str) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    static EXTRACT_LOCK: Mutex<()> = Mutex::new(());

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(exe) {
            return cached.clone();
        }
    }

    let _serialized = EXTRACT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 拿到锁后二次确认：等锁期间可能已经有人把同样的图标抽好了。
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(exe) {
            return cached.clone();
        }
    }

    let value = extract_icon(exe);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(exe.to_string(), value.clone());
    }
    value
}

fn extract_icon(exe: &str) -> Option<String> {
    if exe.trim().is_empty() {
        return None;
    }
    let wide: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut info = SHFILEINFOW::default();
        let ret = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ret == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let result = hicon_to_png(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        result
    }
}

/// `HICON` → 32 位 BGRA 位图 → RGBA → PNG DataURL。
unsafe fn hicon_to_png(hicon: HICON) -> Option<String> {
    let mut icon_info = ICONINFO::default();
    GetIconInfo(hicon, &mut icon_info).ok()?;

    let color = icon_info.hbmColor;
    let mask = icon_info.hbmMask;
    let png = if color.is_invalid() {
        None
    } else {
        bitmap_to_png(color)
    };

    if !color.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(color.0));
    }
    if !mask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(mask.0));
    }
    png
}

unsafe fn bitmap_to_png(bitmap: HBITMAP) -> Option<String> {
    let mut bmp = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(bitmap.0),
        size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut c_void),
    ) == 0
    {
        return None;
    }
    let width = bmp.bmWidth.max(1);
    let height = bmp.bmHeight.max(1);

    let screen_dc: HDC = GetDC(None);
    if screen_dc.is_invalid() {
        return None;
    }
    let mem_dc = CreateCompatibleDC(Some(screen_dc));

    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height, // 负数 = top-down，省去上下翻转
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        ..Default::default()
    };

    let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
    let lines = GetDIBits(
        mem_dc,
        bitmap,
        0,
        height as u32,
        Some(buffer.as_mut_ptr() as *mut c_void),
        &mut info,
        DIB_RGB_COLORS,
    );

    let _ = DeleteDC(mem_dc);
    ReleaseDC(None, screen_dc);

    if lines == 0 {
        return None;
    }

    // 部分图标没有 alpha 通道（全 0），此时需要用颜色判断可见性。
    let has_alpha = buffer.chunks_exact(4).any(|px| px[3] != 0);
    let mut rgba = Vec::with_capacity(buffer.len());
    for px in buffer.chunks_exact(4) {
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        rgba.push(r);
        rgba.push(g);
        rgba.push(b);
        rgba.push(if has_alpha {
            a
        } else if r | g | b == 0 {
            0
        } else {
            255
        });
    }

    let image = image::RgbaImage::from_raw(width as u32, height as u32, rgba)?;
    let mut encoded = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut encoded), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&encoded)
    ))
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_reports_windows() {
        assert_eq!(platform_id(), "windows");
    }

    #[test]
    fn elevation_probe_does_not_panic() {
        // 结果取决于运行环境，这里只保证调用链可用。
        let elevated = is_elevated();
        println!("elevated = {elevated}");
    }

    #[test]
    fn window_enumeration_returns_owned_titles() {
        // 有桌面会话时通常能枚举到若干窗口；无会话时返回空表也不应 panic。
        let titles = window_titles();
        println!("enumerated windows for {} processes", titles.len());
        for (pid, list) in titles.iter().take(3) {
            assert!(*pid > 0);
            assert!(list.iter().all(|title| !title.trim().is_empty()));
        }
    }

    /// 一组几乎必然存在的系统可执行文件，用来做图标抽取测试。
    const ICON_CANDIDATES: [&str; 4] = [
        r"C:\Windows\System32\notepad.exe",
        r"C:\Windows\explorer.exe",
        r"C:\Windows\System32\cmd.exe",
        r"C:\Windows\System32\mspaint.exe",
    ];

    fn existing_candidates() -> Vec<&'static str> {
        ICON_CANDIDATES
            .iter()
            .copied()
            .filter(|path| std::path::Path::new(path).exists())
            .collect()
    }

    #[test]
    fn extracts_png_data_url_for_system_executables() {
        let candidates = existing_candidates();
        if candidates.is_empty() {
            eprintln!("跳过：找不到可用于测试的系统可执行文件");
            return;
        }

        for exe in candidates {
            let data_url = icon_data_url(exe).unwrap_or_else(|| panic!("未能抽取 {exe} 的图标"));
            assert!(
                data_url.starts_with("data:image/png;base64,"),
                "图标应为 PNG DataURL，实际前缀：{}",
                &data_url[..data_url.len().min(40)]
            );
            assert!(
                data_url.len() > 200,
                "{exe} 的 PNG 数据看起来太小，可能为空图"
            );
        }
    }

    #[test]
    fn icon_results_are_cached() {
        let Some(exe) = existing_candidates().first().copied() else {
            return;
        };
        let first = icon_data_url(exe);
        let second = icon_data_url(exe);
        assert_eq!(first, second);
    }

    /// 图标抽取必须在多线程下稳定成功。
    ///
    /// 这条测试是为一个真实缺陷加的：`SHGetFileInfoW` 并发进入时会偶发返回空图标，
    /// 而 Tauri 的命令处理器本身就是多线程的，所以必须串行化提取过程。
    #[test]
    fn icon_extraction_is_thread_safe() {
        let candidates = existing_candidates();
        if candidates.is_empty() {
            eprintln!("跳过：找不到可用于测试的系统可执行文件");
            return;
        }

        let mut handles = Vec::new();
        for _ in 0..8 {
            let exes: Vec<&'static str> = candidates.clone();
            handles.push(std::thread::spawn(move || {
                for exe in exes {
                    let icon = icon_data_url(exe);
                    assert!(icon.is_some(), "并发抽取 {exe} 的图标失败");
                }
            }));
        }
        for handle in handles {
            handle.join().expect("图标抽取线程 panic");
        }
    }

    #[test]
    fn access_probe_is_consistent_with_self() {
        // 自己一定能被自己打开。
        assert!(is_process_accessible(std::process::id()));
        assert!(!is_process_accessible(0));
    }
}
