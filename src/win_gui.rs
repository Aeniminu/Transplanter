#![allow(unsafe_op_in_unsafe_fn)]

use std::env;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::ide_support::write_manifest;
use crate::paths::display_path;
use crate::project::{
    FileStamp, compile_project_file, output_path_for, snapshot_output_files, snapshot_source_files,
    sync_project,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    DrawTextW, EndPaint, FF_DONTCARE, FW_NORMAL, FillRect, FrameRect, HBRUSH, HDC, HFONT,
    InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, ScreenToClient, SetBkColor, SetBkMode,
    SetTextColor, SetWindowRgn, TRANSPARENT, UpdateWindow,
};
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows_sys::Win32::System::Console::FreeConsole;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
use windows_sys::Win32::UI::Shell::{
    BIF_EDITBOX, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW,
    SHGetPathFromIDListW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BS_OWNERDRAW, CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    ES_AUTOHSCROLL, GWLP_USERDATA, GetCursorPos, GetDlgCtrlID, GetMessageW, GetWindowLongPtrW,
    GetWindowTextW, HMENU, HTCAPTION, HTCLIENT, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage,
    RegisterClassW, SW_MINIMIZE, SW_SHOW, SendMessageW, SetTimer, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN,
    WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_NCDESTROY,
    WM_NCHITTEST, WM_PAINT, WM_SETFONT, WM_TIMER, WNDCLASSW, WS_CHILD, WS_POPUP, WS_TABSTOP,
    WS_VISIBLE,
};

const CLASS_NAME: &str = "transplanter_window";
const WINDOW_TITLE: &str = "Transplanter / 耕訳機";
const CONFIG_FILE_NAME: &str = "transplanter.toml";
const DEFAULT_MAIN_RS: &str = r#"use transplanter_rust::prelude::*;

fn main() {
    loop {
        if can_harvest() {
            harvest();
        } else {
            move_dir(Direction::East);
        }
    }
}
"#;

const ID_SRC_EDIT: i32 = 101;
const ID_OUT_EDIT: i32 = 102;
const ID_SRC_BROWSE: i32 = 201;
const ID_OUT_BROWSE: i32 = 202;
const ID_MINIMIZE: i32 = 204;
const ID_CLOSE: i32 = 205;
const ID_SRC_LABEL: i32 = 401;
const ID_OUT_LABEL: i32 = 402;

const TIMER_ID: usize = 1;
const TIMER_INTERVAL_MS: u32 = 250;
const WINDOW_WIDTH: i32 = 520;
const WINDOW_HEIGHT: i32 = 248;
const TITLE_HEIGHT: i32 = 40;

const COLOR_BACKGROUND: u32 = rgb(41, 41, 41);
const COLOR_PANEL: u32 = rgb(41, 41, 41);
const COLOR_TITLE: u32 = rgb(85, 85, 85);
const COLOR_BORDER: u32 = rgb(72, 72, 72);
const COLOR_TEXT: u32 = rgb(225, 225, 225);
const COLOR_MUTED: u32 = rgb(202, 202, 202);
const COLOR_ACCENT: u32 = rgb(170, 204, 0);
const COLOR_BUTTON: u32 = rgb(104, 126, 0);
const COLOR_BUTTON_DOWN: u32 = rgb(83, 101, 0);
const COLOR_EDIT: u32 = rgb(33, 33, 33);

const CLOSE_ICON: [&str; 9] = [
    "##.....##",
    ".##...##.",
    "..##.##..",
    "...###...",
    "....#....",
    "...###...",
    "..##.##..",
    ".##...##.",
    "##.....##",
];

const MINIMIZE_ICON: [&str; 3] = ["###########", "###########", "###########"];

const FOLDER_ICON: [&str; 9] = [
    "..........",
    ".####.....",
    ".#######..",
    ".########.",
    ".##....##.",
    ".##....##.",
    ".########.",
    ".########.",
    "..........",
];

const DOWN_ARROW_ICON: [&str; 11] = [
    "....###....",
    "....###....",
    "....###....",
    "....###....",
    "....###....",
    "###########",
    ".#########.",
    "..#######..",
    "...#####...",
    "....###....",
    ".....#.....",
];

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

thread_local! {
    static THEME: Theme = unsafe { Theme::new() };
}

struct Theme {
    background: HBRUSH,
    panel: HBRUSH,
    title: HBRUSH,
    border: HBRUSH,
    button: HBRUSH,
    button_down: HBRUSH,
    edit: HBRUSH,
    icon: HBRUSH,
    accent: HBRUSH,
    font: HFONT,
}

impl Theme {
    unsafe fn new() -> Self {
        let font_name = wide("Yu Gothic UI");
        Self {
            background: CreateSolidBrush(COLOR_BACKGROUND),
            panel: CreateSolidBrush(COLOR_PANEL),
            title: CreateSolidBrush(COLOR_TITLE),
            border: CreateSolidBrush(COLOR_BORDER),
            button: CreateSolidBrush(COLOR_BUTTON),
            button_down: CreateSolidBrush(COLOR_BUTTON_DOWN),
            edit: CreateSolidBrush(COLOR_EDIT),
            icon: CreateSolidBrush(COLOR_TEXT),
            accent: CreateSolidBrush(COLOR_ACCENT),
            font: CreateFontW(
                -15,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                font_name.as_ptr(),
            ),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Config {
    src_dir: String,
    out_dir: String,
}

struct GuiState {
    config: Config,
    config_path: PathBuf,
    startup_error: Option<String>,
    watcher: Option<WatchHandle>,
    tx: mpsc::Sender<GuiEvent>,
    rx: mpsc::Receiver<GuiEvent>,
    last_src_text: String,
    last_out_text: String,
    active: bool,
    spinner: usize,
}

struct WatchHandle {
    src_dir: PathBuf,
    out_dir: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Copy)]
struct ControlRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl ControlRect {
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

enum GuiEvent {
    Status(String),
    Error(String),
}

pub fn detach_console() {
    unsafe {
        FreeConsole();
    }
}

pub fn run() -> Result<(), String> {
    unsafe {
        CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
    }

    let result = unsafe { run_window() };

    unsafe {
        CoUninitialize();
    }

    result
}

unsafe fn run_window() -> Result<(), String> {
    let instance = GetModuleHandleW(null());
    let class_name = wide(CLASS_NAME);
    let title = wide(WINDOW_TITLE);
    let cursor = LoadCursorW(null_mut(), IDC_ARROW);

    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: cursor,
        hbrBackground: with_theme(|theme| theme.background),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
    };

    RegisterClassW(&wc);

    let state = Box::new(GuiState::new());
    let state_ptr = Box::into_raw(state);
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        i32::MIN,
        i32::MIN,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        null_mut(),
        null_mut(),
        instance,
        state_ptr.cast::<c_void>(),
    );

    if hwnd.is_null() {
        let _ = Box::from_raw(state_ptr);
        return Err("エラー: Transplanter のウィンドウを作成できませんでした".to_string());
    }

    let rounded = CreateRoundRectRgn(0, 0, WINDOW_WIDTH + 1, WINDOW_HEIGHT + 1, 10, 10);
    if !rounded.is_null() {
        SetWindowRgn(hwnd, rounded, 1);
    }

    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    Ok(())
}

impl GuiState {
    fn new() -> Self {
        let config_path = config_path();
        let (config, startup_error) = load_or_create_initial_workspace(&config_path);
        let (tx, rx) = mpsc::channel();
        Self {
            last_src_text: config.src_dir.clone(),
            last_out_text: config.out_dir.clone(),
            config,
            config_path,
            startup_error,
            watcher: None,
            tx,
            rx,
            active: false,
            spinner: 0,
        }
    }
}

impl WatchHandle {
    fn matches(&self, src_dir: &Path, out_dir: &Path) -> bool {
        self.src_dir == src_dir && self.out_dir == out_dir
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = create.lpCreateParams as *mut GuiState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            create_controls(hwnd, &mut *state_ptr);
            SetTimer(hwnd, TIMER_ID, TIMER_INTERVAL_MS, None);
            if (*state_ptr).startup_error.is_none() {
                save_config_and_start(hwnd);
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as i32;
            match id {
                ID_SRC_BROWSE => browse_and_set_path(hwnd, ID_SRC_EDIT, true),
                ID_OUT_BROWSE => browse_and_set_path(hwnd, ID_OUT_EDIT, false),
                ID_MINIMIZE => {
                    ShowWindow(hwnd, SW_MINIMIZE);
                }
                ID_CLOSE => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            if wparam == TIMER_ID {
                save_if_edits_changed(hwnd);
                tick_spinner(hwnd);
                drain_events(hwnd);
            }
            0
        }
        WM_PAINT => {
            paint_window(hwnd);
            0
        }
        WM_NCHITTEST => hit_test(hwnd),
        WM_ERASEBKGND => 1,
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => color_control(msg, wparam, lparam),
        WM_DRAWITEM => {
            draw_button(lparam);
            1
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            if let Some(state) = state_from_hwnd(hwnd) {
                stop_watcher(state);
            }
            PostQuitMessage(0);
            0
        }
        WM_NCDESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiState;
            if !state_ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = Box::from_raw(state_ptr);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn create_controls(hwnd: HWND, state: &mut GuiState) {
    create_control(
        hwnd,
        "BUTTON",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ControlRect::new(452, 6, 29, 28),
        ID_MINIMIZE,
    );
    create_control(
        hwnd,
        "BUTTON",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ControlRect::new(486, 6, 29, 28),
        ID_CLOSE,
    );
    create_control(
        hwnd,
        "STATIC",
        "rs_src のパス",
        WS_CHILD | WS_VISIBLE,
        ControlRect::new(20, 60, 180, 20),
        ID_SRC_LABEL,
    );
    let src_edit = create_control(
        hwnd,
        "EDIT",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
        ControlRect::new(20, 86, 420, 26),
        ID_SRC_EDIT,
    );
    create_control(
        hwnd,
        "BUTTON",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ControlRect::new(452, 86, 42, 26),
        ID_SRC_BROWSE,
    );

    create_control(
        hwnd,
        "STATIC",
        "ゲームの Save フォルダ",
        WS_CHILD | WS_VISIBLE,
        ControlRect::new(20, 165, 220, 20),
        ID_OUT_LABEL,
    );
    let out_edit = create_control(
        hwnd,
        "EDIT",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
        ControlRect::new(20, 191, 420, 26),
        ID_OUT_EDIT,
    );
    create_control(
        hwnd,
        "BUTTON",
        "",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ControlRect::new(452, 191, 42, 26),
        ID_OUT_BROWSE,
    );

    set_window_text(src_edit, &state.config.src_dir);
    set_window_text(out_edit, &state.config.out_dir);
    if let Some(error) = &state.startup_error {
        set_status(hwnd, error);
    }
}

unsafe fn create_control(
    parent: HWND,
    class_name: &str,
    text: &str,
    style: u32,
    bounds: ControlRect,
    id: i32,
) -> HWND {
    let class_name = wide(class_name);
    let text = wide(text);
    let control = CreateWindowExW(
        0,
        class_name.as_ptr(),
        text.as_ptr(),
        style,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        parent,
        id as isize as HMENU,
        GetModuleHandleW(null()),
        null_mut(),
    );
    SendMessageW(
        control,
        WM_SETFONT,
        with_theme(|theme| theme.font) as usize,
        1,
    );
    control
}

unsafe fn paint_window(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let arrow_offset = state_from_hwnd(hwnd)
        .filter(|state| state.active)
        .map(|state| (state.spinner % 3) as i32)
        .unwrap_or(0);

    with_theme(|theme| {
        let background = RECT {
            left: 0,
            top: 0,
            right: WINDOW_WIDTH,
            bottom: WINDOW_HEIGHT,
        };
        FillRect(hdc, &background, theme.panel);
        FrameRect(hdc, &background, theme.border);

        let title = RECT {
            left: 1,
            top: 1,
            right: WINDOW_WIDTH - 1,
            bottom: TITLE_HEIGHT,
        };
        FillRect(hdc, &title, theme.title);

        let arrow = RECT {
            left: 0,
            top: 126 + arrow_offset,
            right: WINDOW_WIDTH,
            bottom: 160 + arrow_offset,
        };
        draw_mask_icon(hdc, &arrow, &DOWN_ARROW_ICON, 3, theme.accent);
    });

    EndPaint(hwnd, &ps);
}

unsafe fn hit_test(hwnd: HWND) -> LRESULT {
    let mut point: POINT = std::mem::zeroed();
    if GetCursorPos(&mut point) != 0 {
        ScreenToClient(hwnd, &mut point);
        if point.y >= 0 && point.y < TITLE_HEIGHT && point.x < WINDOW_WIDTH - 76 {
            return HTCAPTION as LRESULT;
        }
    }

    HTCLIENT as LRESULT
}

unsafe fn color_control(msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hdc = wparam as HDC;
    let control = lparam as HWND;
    let id = GetDlgCtrlID(control);

    with_theme(|theme| {
        SetBkMode(hdc, TRANSPARENT as i32);
        match msg {
            WM_CTLCOLOREDIT => {
                SetTextColor(hdc, COLOR_TEXT);
                SetBkColor(hdc, COLOR_EDIT);
                theme.edit as LRESULT
            }
            WM_CTLCOLORSTATIC => {
                if id == ID_SRC_LABEL || id == ID_OUT_LABEL {
                    SetTextColor(hdc, COLOR_ACCENT);
                    SetBkColor(hdc, COLOR_PANEL);
                    theme.panel as LRESULT
                } else {
                    SetTextColor(hdc, COLOR_MUTED);
                    SetBkColor(hdc, COLOR_PANEL);
                    theme.panel as LRESULT
                }
            }
            WM_CTLCOLORBTN => {
                SetTextColor(hdc, COLOR_TEXT);
                SetBkColor(hdc, COLOR_BUTTON);
                theme.button as LRESULT
            }
            _ => theme.panel as LRESULT,
        }
    })
}

unsafe fn draw_button(lparam: LPARAM) {
    let item = &*(lparam as *const DRAWITEMSTRUCT);
    let selected = item.itemState & ODS_SELECTED != 0;
    let id = GetDlgCtrlID(item.hwndItem);

    with_theme(|theme| {
        let brush = if selected {
            theme.button_down
        } else {
            theme.button
        };
        FillRect(item.hDC, &item.rcItem, brush);
        FrameRect(item.hDC, &item.rcItem, theme.border);

        match id {
            ID_MINIMIZE => {
                draw_mask_icon(item.hDC, &item.rcItem, &MINIMIZE_ICON, 2, theme.icon);
                return;
            }
            ID_CLOSE => {
                draw_mask_icon(item.hDC, &item.rcItem, &CLOSE_ICON, 2, theme.icon);
                return;
            }
            ID_SRC_BROWSE | ID_OUT_BROWSE => {
                draw_mask_icon(item.hDC, &item.rcItem, &FOLDER_ICON, 2, theme.icon);
                return;
            }
            _ => {}
        }

        let mut text_rect = item.rcItem;
        let text = get_window_text(item.hwndItem);
        let text = wide(&text);
        SetBkMode(item.hDC, TRANSPARENT as i32);
        SetTextColor(item.hDC, COLOR_TEXT);
        DrawTextW(
            item.hDC,
            text.as_ptr(),
            -1,
            &mut text_rect,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
    });
}

unsafe fn draw_mask_icon(hdc: HDC, bounds: &RECT, icon: &[&str], scale: i32, brush: HBRUSH) {
    let icon_width = icon.iter().map(|row| row.len()).max().unwrap_or_default() as i32 * scale;
    let icon_height = icon.len() as i32 * scale;
    let left = bounds.left + ((bounds.right - bounds.left) - icon_width) / 2;
    let top = bounds.top + ((bounds.bottom - bounds.top) - icon_height) / 2;

    for (row_index, row) in icon.iter().enumerate() {
        for (column_index, pixel) in row.as_bytes().iter().enumerate() {
            if *pixel != b'#' {
                continue;
            }
            let x = left + column_index as i32 * scale;
            let y = top + row_index as i32 * scale;
            let rect = RECT {
                left: x,
                top: y,
                right: x + scale,
                bottom: y + scale,
            };
            FillRect(hdc, &rect, brush);
        }
    }
}

unsafe fn browse_and_set_path(hwnd: HWND, edit_id: i32, is_src: bool) {
    let Some(path) = choose_folder(hwnd) else {
        return;
    };
    set_control_text(hwnd, edit_id, &path);

    if let Some(state) = state_from_hwnd(hwnd) {
        if is_src {
            state.config.src_dir = path.clone();
            state.last_src_text = path;
        } else {
            state.config.out_dir = path.clone();
            state.last_out_text = path;
        }
    }

    save_config_and_start(hwnd);
}

unsafe fn choose_folder(hwnd: HWND) -> Option<String> {
    let mut display_name = vec![0u16; 260];
    let title = wide("フォルダを選択してください");
    let browse = BROWSEINFOW {
        hwndOwner: hwnd,
        pidlRoot: null_mut(),
        pszDisplayName: display_name.as_mut_ptr(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        lpfn: None,
        lParam: 0,
        iImage: 0,
    };

    let pidl = SHBrowseForFolderW(&browse);
    if pidl.is_null() {
        return None;
    }

    let mut path_buf = vec![0u16; 32768];
    let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr()) != 0;
    CoTaskMemFree(pidl.cast::<c_void>());
    if !ok {
        return None;
    }

    Some(string_from_wide_buffer(&path_buf))
}

unsafe fn save_if_edits_changed(hwnd: HWND) {
    let src = get_control_text(hwnd, ID_SRC_EDIT);
    let out = get_control_text(hwnd, ID_OUT_EDIT);
    let changed = if let Some(state) = state_from_hwnd(hwnd) {
        if state.last_src_text != src || state.last_out_text != out {
            state.last_src_text = src.clone();
            state.last_out_text = out.clone();
            state.config.src_dir = src;
            state.config.out_dir = out;
            true
        } else {
            false
        }
    } else {
        false
    };

    if changed {
        save_config_and_start(hwnd);
    }
}

unsafe fn save_config_and_start(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    let config = Config {
        src_dir: state.config.src_dir.trim().to_string(),
        out_dir: state.config.out_dir.trim().to_string(),
    };
    state.config = config.clone();

    if let Err(err) = write_config(&state.config_path, &config) {
        stop_watcher(state);
        state.active = false;
        set_status(hwnd, &err);
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    if config.src_dir.is_empty() || config.out_dir.is_empty() {
        stop_watcher(state);
        state.active = false;
        set_status(hwnd, "待機中: Saveフォルダを選択");
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    let src_dir = PathBuf::from(&config.src_dir);
    let out_dir = PathBuf::from(&config.out_dir);

    if !src_dir.is_dir() {
        stop_watcher(state);
        state.active = false;
        set_status(hwnd, "エラー: rs_src が見つかりません");
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    if let Err(err) = fs::create_dir_all(&out_dir) {
        stop_watcher(state);
        state.active = false;
        set_status(
            hwnd,
            &format!("エラー: Save フォルダを作成できません: {err}"),
        );
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    if state
        .watcher
        .as_ref()
        .is_some_and(|watcher| watcher.matches(&src_dir, &out_dir))
    {
        state.active = true;
        return;
    }

    stop_watcher(state);
    let stop = Arc::new(AtomicBool::new(false));
    let tx = state.tx.clone();
    let thread_stop = Arc::clone(&stop);
    let thread_src = src_dir.clone();
    let thread_out = out_dir.clone();
    let thread = thread::spawn(move || watch_loop(thread_src, thread_out, thread_stop, tx));
    state.watcher = Some(WatchHandle {
        src_dir,
        out_dir,
        stop,
        thread: Some(thread),
    });
    state.active = true;
    set_status(hwnd, "監視中");
}

fn watch_loop(
    src_dir: PathBuf,
    out_dir: PathBuf,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<GuiEvent>,
) {
    match sync_project(&src_dir, &out_dir) {
        Ok(count) => send_status(&tx, format!("OK: {count} 件を変換しました")),
        Err(err) => send_error(&tx, err),
    }

    let mut seen_sources = snapshot_sources_or_report(&src_dir, &tx);
    let mut seen_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &seen_sources, &tx);

    while !stop.load(Ordering::Relaxed) {
        sleep_until_next_poll(&stop);
        if stop.load(Ordering::Relaxed) {
            break;
        }

        let current_sources = snapshot_sources_or_report(&src_dir, &tx);
        let current_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &current_sources, &tx);

        if current_sources.keys().ne(seen_sources.keys())
            && let Err(err) = write_manifest(&src_dir)
        {
            send_error(&tx, err);
        }

        for (input_path, stamp) in &current_sources {
            let Ok(output_path) = output_path_for(&src_dir, &out_dir, input_path) else {
                continue;
            };
            let source_changed = seen_sources.get(input_path) != Some(stamp);
            let output_changed =
                seen_outputs.get(&output_path) != current_outputs.get(&output_path);

            if source_changed || output_changed {
                match compile_project_file(&src_dir, &out_dir, input_path) {
                    Ok(output_path) => send_status(
                        &tx,
                        format!("OK: {} を更新しました", file_name_for_status(&output_path)),
                    ),
                    Err(err) => send_error(&tx, err),
                }
            }
        }

        seen_sources = current_sources;
        seen_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &seen_sources, &tx);
    }
}

fn snapshot_sources_or_report(
    src_dir: &Path,
    tx: &mpsc::Sender<GuiEvent>,
) -> std::collections::BTreeMap<PathBuf, FileStamp> {
    match snapshot_source_files(src_dir) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            send_error(tx, err);
            std::collections::BTreeMap::new()
        }
    }
}

fn snapshot_outputs_or_report(
    src_dir: &Path,
    out_dir: &Path,
    sources: &std::collections::BTreeMap<PathBuf, FileStamp>,
    tx: &mpsc::Sender<GuiEvent>,
) -> std::collections::BTreeMap<PathBuf, Option<FileStamp>> {
    match snapshot_output_files(src_dir, out_dir, sources.keys()) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            send_error(tx, err);
            std::collections::BTreeMap::new()
        }
    }
}

fn sleep_until_next_poll(stop: &AtomicBool) {
    for _ in 0..10 {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn send_status(tx: &mpsc::Sender<GuiEvent>, message: String) {
    let _ = tx.send(GuiEvent::Status(message));
}

fn send_error(tx: &mpsc::Sender<GuiEvent>, message: String) {
    let message = compact_error_message(&message);
    let message = if message.starts_with("エラー:") {
        message
    } else {
        format!("エラー: {message}")
    };
    let _ = tx.send(GuiEvent::Error(message));
}

fn file_name_for_status(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| display_path(path))
}

fn compact_error_message(message: &str) -> String {
    let Some(body) = message.strip_prefix("エラー: ") else {
        return message.to_string();
    };

    for marker in [".rs:", ".py:"] {
        if let Some(marker_pos) = body.find(marker) {
            let path_end = marker_pos + marker.len() - 1;
            let path = &body[..path_end];
            let rest = &body[path_end..];
            let file_name = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path);
            return format!("エラー: {file_name}{rest}");
        }
    }

    message.to_string()
}

unsafe fn tick_spinner(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    if !state.active {
        return;
    }

    state.spinner = (state.spinner + 1) % 3;
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn drain_events(hwnd: HWND) {
    let events = if let Some(state) = state_from_hwnd(hwnd) {
        let mut events = Vec::new();
        while let Ok(event) = state.rx.try_recv() {
            events.push(event);
        }
        events
    } else {
        Vec::new()
    };

    for event in events {
        match event {
            GuiEvent::Status(message) => set_status(hwnd, &message),
            GuiEvent::Error(message) => set_status(hwnd, &message),
        }
    }
}

unsafe fn stop_watcher(state: &mut GuiState) {
    if let Some(watcher) = state.watcher.take() {
        watcher.stop();
    }
}

unsafe fn state_from_hwnd(hwnd: HWND) -> Option<&'static mut GuiState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiState;
    ptr.as_mut()
}

unsafe fn set_status(_hwnd: HWND, _text: &str) {}

unsafe fn get_control_text(parent: HWND, id: i32) -> String {
    let control = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id);
    get_window_text(control)
}

unsafe fn set_control_text(parent: HWND, id: i32, text: &str) {
    let control = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id);
    if control.is_null() {
        return;
    }
    set_window_text(control, text);
    InvalidateRect(control, null(), 1);
}

unsafe fn get_window_text(hwnd: HWND) -> String {
    let mut buffer = vec![0u16; 32768];
    let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

unsafe fn set_window_text(hwnd: HWND, text: &str) {
    let text = wide(text);
    SetWindowTextW(hwnd, text.as_ptr());
}

fn config_path() -> PathBuf {
    exe_dir().join(CONFIG_FILE_NAME)
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn load_or_create_initial_workspace(config_path: &Path) -> (Config, Option<String>) {
    let config_exists = config_path.is_file();
    let config = if config_exists {
        match read_config(config_path) {
            Ok(config) => config,
            Err(err) => return (Config::default(), Some(err)),
        }
    } else {
        default_initial_config(config_path)
    };

    match ensure_initial_workspace(config_path, &config, config_exists) {
        Ok(()) => (config, None),
        Err(err) => (config, Some(err)),
    }
}

fn default_initial_config(config_path: &Path) -> Config {
    let base_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Config {
        src_dir: base_dir.join("rs_src").to_string_lossy().into_owned(),
        out_dir: String::new(),
    }
}

fn ensure_initial_workspace(
    config_path: &Path,
    config: &Config,
    config_exists: bool,
) -> Result<(), String> {
    if !config_exists {
        write_config(config_path, config)?;
    }

    if config.src_dir.trim().is_empty() {
        return Ok(());
    }

    let src_dir = PathBuf::from(&config.src_dir);
    fs::create_dir_all(&src_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(&src_dir)
        )
    })?;

    let main_path = src_dir.join("main.rs");
    if !main_path.exists() {
        fs::write(&main_path, DEFAULT_MAIN_RS).map_err(|err| {
            format!(
                "エラー: `{}` に書き込めません: {err}",
                display_path(&main_path)
            )
        })?;
    }

    write_manifest(&src_dir)?;
    Ok(())
}

fn read_config(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("エラー: `{}` を読み込めません: {err}", display_path(path)))?;
    parse_config(&contents)
}

fn write_config(path: &Path, config: &Config) -> Result<(), String> {
    let contents = render_config(config);
    fs::write(path, contents)
        .map_err(|err| format!("エラー: `{}` に書き込めません: {err}", display_path(path)))
}

fn render_config(config: &Config) -> String {
    format!(
        "src_dir = {}\nout_dir = {}\n",
        toml_string(&config.src_dir),
        toml_string(&config.out_dir)
    )
}

fn parse_config(contents: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_toml_string(value.trim())?;
        match key {
            "src_dir" => config.src_dir = value,
            "out_dir" => config.out_dir = value,
            _ => {}
        }
    }
    Ok(config)
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(ch),
        }
    }
    output.push('"');
    output
}

fn parse_toml_string(value: &str) -> Result<String, String> {
    let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err("エラー: 設定ファイルの文字列は \"...\" で囲んでください".to_string());
    };

    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err("エラー: 設定ファイルの文字列エスケープが途中で終わっています".to_string());
        };
        match escaped {
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            other => output.push(other),
        }
    }

    Ok(output)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn string_from_wide_buffer(buffer: &[u16]) -> String {
    let len = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

fn with_theme<R>(f: impl FnOnce(&Theme) -> R) -> R {
    THEME.with(|theme| f(theme))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn toml_string_escapes_windows_paths() {
        assert_eq!(
            toml_string(r#"C:\Users\Player\The "Farm""#),
            r#""C:\\Users\\Player\\The \"Farm\"""#
        );
    }

    #[test]
    fn config_round_trips_paths() {
        let config = Config {
            src_dir: r"C:\Users\Player\Desktop\farming\rs_src".to_string(),
            out_dir: r"C:\Users\Player\AppData\LocalLow\TheFarmerWasReplaced\Saves\Rust"
                .to_string(),
        };
        let rendered = render_config(&config);
        assert_eq!(parse_config(&rendered).unwrap(), config);
    }

    #[test]
    fn initial_setup_creates_project_files() {
        let workspace = temp_workspace("initial_setup");
        let config_path = workspace.join("transplanter.toml");

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(PathBuf::from(&config.src_dir), workspace.join("rs_src"));
        assert_eq!(config.out_dir, "");
        assert!(config_path.is_file());
        assert!(workspace.join("rs_src").join("main.rs").is_file());
        assert!(
            fs::read_to_string(workspace.join("rs_src").join("main.rs"))
                .unwrap()
                .contains("move_dir(Direction::East);")
        );
        assert!(workspace.join("Cargo.toml").is_file());
        assert!(!workspace.join("rs_src").join("Cargo.toml").exists());
        assert!(
            workspace
                .join(".transplanter_ide")
                .join("transplanter_rust")
                .join("src")
                .join("prelude.rs")
                .is_file()
        );
        assert!(!workspace.join("rs_src").join(".transplanter_ide").exists());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn initial_setup_preserves_existing_main_rs() {
        let workspace = temp_workspace("initial_setup_preserve");
        let src_dir = workspace.join("rs_src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    quick_print(7);\n}\n",
        )
        .unwrap();

        let (_config, startup_error) =
            load_or_create_initial_workspace(&workspace.join("transplanter.toml"));

        assert_eq!(startup_error, None);
        assert_eq!(
            fs::read_to_string(src_dir.join("main.rs")).unwrap(),
            "fn main() {\n    quick_print(7);\n}\n"
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn watch_loop_reports_compile_errors() {
        let workspace = temp_workspace("compile_error");
        let src_dir = workspace.join("rs_src");
        let out_dir = workspace.join("save");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {\n    harvest()\n}\n").unwrap();
        let output_path = out_dir.join("main.py");
        fs::write(&output_path, "harvest()\n").unwrap();

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || watch_loop(src_dir, out_dir, thread_stop, tx));

        let message = recv_event_text(&rx);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "harvest()\n");
        let _ = fs::remove_dir_all(workspace);

        assert!(message.contains("式文の後に `;` が必要です"), "{message}");
    }

    #[test]
    fn watch_loop_reports_rust_check_errors_without_overwriting_output() {
        let workspace = temp_workspace("rust_check_error");
        let src_dir = workspace.join("rs_src");
        let out_dir = workspace.join("save");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n    missing_game_api();\n}\n",
        )
        .unwrap();
        let output_path = out_dir.join("main.py");
        fs::write(&output_path, "harvest()\n").unwrap();

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || watch_loop(src_dir, out_dir, thread_stop, tx));

        let message = recv_event_text(&rx);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "harvest()\n");
        let _ = fs::remove_dir_all(workspace);

        assert!(message.contains("missing_game_api"), "{message}");
    }

    #[test]
    fn watch_loop_reports_output_directory_errors() {
        let workspace = temp_workspace("write_error");
        let src_dir = workspace.join("rs_src");
        let out_dir = workspace.join("save");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {\n    harvest();\n}\n").unwrap();
        fs::write(&out_dir, "this is a file, not a directory").unwrap();

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || watch_loop(src_dir, out_dir, thread_stop, tx));

        let message = recv_event_text(&rx);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        let _ = fs::remove_dir_all(workspace);

        assert!(message.contains("作成できません"), "{message}");
    }

    fn recv_event_text(rx: &mpsc::Receiver<GuiEvent>) -> String {
        match rx.recv_timeout(Duration::from_secs(3)).unwrap() {
            GuiEvent::Status(message) | GuiEvent::Error(message) => message,
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "transplanter_gui_{name}_{}_{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
