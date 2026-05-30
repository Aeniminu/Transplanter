#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::ide_support::write_manifest;
use crate::language::LanguageMode;
use crate::paths::display_path;
use crate::project::{
    FileStamp, compile_project_file, output_path_for, snapshot_output_files, snapshot_source_files,
    sync_project,
};
use crate::updater::{self, ReleaseInfo};
use crate::workspace_setup::{
    Config, config_path, load_or_create_initial_workspace, prepare_existing_workspace,
    prepare_language_workspace, write_config,
};
use windows::Win32::Foundation::HWND as WinHwnd;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree as WinCoTaskMemFree, IBindCtx,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::PCWSTR;
use windows_sys::Win32::Foundation::{GlobalFree, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_CALCRECT, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FW_BOLD, FillRect, GetDC, HBRUSH, HDC, HFONT,
    HPEN, IntersectClipRect, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_NULL, ReleaseDC,
    RestoreDC, RoundRect, SRCCOPY, SaveDC, ScreenToClient, SelectClipRgn, SelectObject, SetBkColor,
    SetBkMode, SetPixel, SetTextColor, SetWindowRgn, TRANSPARENT, UpdateWindow,
};
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::System::Console::FreeConsole;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    ES_AUTOHSCROLL, GWLP_USERDATA, GetClientRect, GetCursorPos, GetDlgCtrlID, GetMessageW,
    GetWindowLongPtrW, GetWindowTextW, HMENU, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
    HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IDYES, LoadCursorW,
    MB_ICONERROR, MB_ICONINFORMATION, MB_YESNO, MINMAXINFO, MSG, MessageBoxW, MoveWindow,
    PostQuitMessage, RegisterClassW, SW_MINIMIZE, SW_SHOW, SendMessageW, SetTimer,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY,
    WM_NCHITTEST, WM_PAINT, WM_SETFONT, WM_SIZE, WM_TIMER, WNDCLASSW, WS_CHILD, WS_POPUP,
};

const CLASS_NAME: &str = "transplanter_window";
const WINDOW_TITLE: &str = "Transplanter";
const STATUS_UPDATE_AVAILABLE: &str = "新しいバージョンがあります";
const STATUS_UP_TO_DATE: &str = "最新のバージョンです";
const STATUS_CHECKING: &str = "確認中";

const ID_SRC_EDIT: i32 = 101;
const ID_OUT_EDIT: i32 = 102;
const ID_MINIMIZE: i32 = 204;
const ID_CLOSE: i32 = 205;
const ID_RUN: i32 = 206;
const ID_STEP: i32 = 207;
const ID_SRC_LABEL: i32 = 401;
const ID_OUT_LABEL: i32 = 402;
const ID_LANGUAGE_LABEL: i32 = 403;

const TIMER_ID: usize = 1;
const TIMER_INTERVAL_MS: u32 = 250;
const WINDOW_WIDTH: i32 = 704;
const WINDOW_HEIGHT: i32 = 620;
const MIN_WINDOW_WIDTH: i32 = 220;
const MIN_WINDOW_HEIGHT: i32 = 160;
const TITLE_HEIGHT: i32 = 66;
const EDITOR_LEFT: i32 = 14;
const EDITOR_TOP: i32 = 66;
const GUTTER_RIGHT: i32 = 48;
const CONTENT_LEFT: i32 = 64;
const PATH_EQUAL_LEFT: i32 = 136;
const PATH_VALUE_LEFT: i32 = 154;
const LANGUAGE_EQUAL_LEFT: i32 = 156;
const LANGUAGE_VALUE_LEFT: i32 = 174;
const IMPORT_ROW_TOP: i32 = 88;
const STATUS_ROW_TOP: i32 = 134;
const DIAGNOSTIC_ROW_TOP: i32 = 180;
const SRC_ROW_TOP: i32 = 226;
const OUT_ROW_TOP: i32 = 272;
const LANGUAGE_ROW_TOP: i32 = 318;
const CALL_ROW_TOP: i32 = 410;
const TEXT_ROW_HEIGHT: i32 = 32;
const RESIZE_BORDER: i32 = 8;
const HORIZONTAL_SCROLL_HEIGHT: i32 = 7;
const HORIZONTAL_SCROLL_MIN_THUMB: i32 = 36;
const CF_UNICODETEXT_FORMAT: u32 = 13;

const COLOR_BACKGROUND: u32 = rgb(84, 87, 85);
const COLOR_PANEL: u32 = rgb(84, 87, 85);
const COLOR_TITLE: u32 = rgb(84, 87, 85);
const COLOR_TEXT: u32 = rgb(252, 252, 242);
const COLOR_MUTED: u32 = rgb(190, 193, 187);
const COLOR_KEYWORD: u32 = rgb(255, 185, 43);
const COLOR_BUILTIN: u32 = rgb(238, 255, 92);
const COLOR_BUTTON: u32 = rgb(104, 126, 0);
const COLOR_BUTTON_DOWN: u32 = rgb(83, 101, 0);
const COLOR_RUN_ACTIVE: u32 = rgb(216, 86, 0);
const COLOR_RUN_ACTIVE_DOWN: u32 = rgb(189, 70, 0);
const COLOR_EDIT: u32 = rgb(41, 41, 41);
const COLOR_GUTTER_LINE: u32 = rgb(82, 84, 82);
const COLOR_SCROLL: u32 = rgb(75, 77, 75);
const COLOR_SELECTION: u32 = rgb(92, 98, 96);
const COLOR_TITLE_SHADOW_SOFT: u32 = rgb(80, 83, 81);
const COLOR_TITLE_SHADOW_DEEP: u32 = rgb(75, 78, 76);
const COLOR_EDIT_SHADOW_DEEP: u32 = rgb(36, 36, 36);

const RUN_ICON: [&str; 13] = [
    ".............",
    "..##.........",
    "..####.......",
    "..######.....",
    "..########...",
    "..##########.",
    "..##########.",
    "..########...",
    "..######.....",
    "..####.......",
    "..##.........",
    ".............",
    ".............",
];

const STEP_ICON: [&str; 13] = [
    ".............",
    "..##......##.",
    "..####....##.",
    "..#..##...##.",
    "..#....##.##.",
    "..#......###.",
    "..#......###.",
    "..#....##.##.",
    "..#..##...##.",
    "..####....##.",
    "..##......##.",
    ".............",
    ".............",
];

const STOP_ICON: [&str; 13] = [
    ".............",
    ".............",
    "...#######...",
    "...#######...",
    "...#######...",
    "...#######...",
    "...#######...",
    "...#######...",
    "...#######...",
    "...#######...",
    ".............",
    ".............",
    ".............",
];

const PAUSE_ICON: [&str; 13] = [
    ".............",
    ".............",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    "...###.###...",
    ".............",
    ".............",
    ".............",
];

const MINIMIZE_ICON: [&str; 3] = ["###########", "###########", "###########"];

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
    button: HBRUSH,
    button_down: HBRUSH,
    run_active: HBRUSH,
    run_active_down: HBRUSH,
    edit: HBRUSH,
    gutter_line: HBRUSH,
    scroll: HBRUSH,
    selection: HBRUSH,
    title_shadow_soft: HBRUSH,
    title_shadow_deep: HBRUSH,
    edit_shadow_deep: HBRUSH,
    icon: HBRUSH,
    no_outline: HPEN,
    font: HFONT,
    title_font: HFONT,
    code_font: HFONT,
    code_hover_font: HFONT,
}

impl Theme {
    unsafe fn new() -> Self {
        let ui_font_name = wide("Yu Gothic UI");
        let code_font_name = wide("Cascadia Mono");
        Self {
            background: CreateSolidBrush(COLOR_BACKGROUND),
            panel: CreateSolidBrush(COLOR_PANEL),
            title: CreateSolidBrush(COLOR_TITLE),
            button: CreateSolidBrush(COLOR_BUTTON),
            button_down: CreateSolidBrush(COLOR_BUTTON_DOWN),
            run_active: CreateSolidBrush(COLOR_RUN_ACTIVE),
            run_active_down: CreateSolidBrush(COLOR_RUN_ACTIVE_DOWN),
            edit: CreateSolidBrush(COLOR_EDIT),
            gutter_line: CreateSolidBrush(COLOR_GUTTER_LINE),
            scroll: CreateSolidBrush(COLOR_SCROLL),
            selection: CreateSolidBrush(COLOR_SELECTION),
            title_shadow_soft: CreateSolidBrush(COLOR_TITLE_SHADOW_SOFT),
            title_shadow_deep: CreateSolidBrush(COLOR_TITLE_SHADOW_DEEP),
            edit_shadow_deep: CreateSolidBrush(COLOR_EDIT_SHADOW_DEEP),
            icon: CreateSolidBrush(COLOR_TEXT),
            no_outline: CreatePen(PS_NULL, 0, 0),
            font: CreateFontW(
                -17,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                code_font_name.as_ptr(),
            ),
            title_font: CreateFontW(
                -22,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                code_font_name.as_ptr(),
            ),
            code_font: CreateFontW(
                -20,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                ui_font_name.as_ptr(),
            ),
            code_hover_font: CreateFontW(
                -22,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                ui_font_name.as_ptr(),
            ),
        }
    }
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
    status_text: String,
    diagnostic_text: String,
    active: bool,
    spinner: usize,
    hover_target: Option<HoverTarget>,
    pressed_title_button: Option<TitleButton>,
    text_selection: Option<TextSelection>,
    text_drag: Option<TextDrag>,
    horizontal_scroll: i32,
    horizontal_scroll_metrics: Option<HorizontalScrollMetrics>,
    horizontal_scroll_drag: Option<HorizontalScrollDrag>,
    update_check_started: bool,
    update_busy: bool,
    update: Option<ReleaseInfo>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HoverTarget {
    SrcDir,
    OutDir,
    Language,
    UpdateStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TitleButton {
    Run,
    Step,
    Minimize,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct TextPosition {
    line: usize,
    char_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextSelection {
    anchor: TextPosition,
    active: TextPosition,
}

#[derive(Clone, Copy)]
struct TextDrag {
    start_point: POINT,
    moved: bool,
    pending_target: Option<HoverTarget>,
}

struct WatchHandle {
    src_dir: PathBuf,
    out_dir: PathBuf,
    language: LanguageMode,
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

    fn to_rect(self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }
}

#[derive(Clone, Copy)]
struct WindowLayout {
    width: i32,
    height: i32,
}

impl WindowLayout {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width: width.max(MIN_WINDOW_WIDTH),
            height: height.max(MIN_WINDOW_HEIGHT),
        }
    }

    fn s(self, value: i32) -> i32 {
        value
    }

    fn title_height(self) -> i32 {
        self.s(TITLE_HEIGHT)
    }

    fn editor_left(self) -> i32 {
        self.s(EDITOR_LEFT)
    }

    fn editor_top(self) -> i32 {
        self.s(EDITOR_TOP)
    }

    fn editor_right(self) -> i32 {
        self.width - self.s(14)
    }

    fn editor_bottom(self) -> i32 {
        self.height - self.s(14)
    }

    fn code_right(self) -> i32 {
        self.editor_right() - self.s(8)
    }

    fn value_right(self) -> i32 {
        self.code_right()
    }

    fn content_left(self) -> i32 {
        self.s(CONTENT_LEFT)
    }

    fn gutter_right(self) -> i32 {
        self.s(GUTTER_RIGHT)
    }

    fn text_row_height(self) -> i32 {
        self.s(TEXT_ROW_HEIGHT).max(1)
    }

    fn horizontal_scroll_height(self) -> i32 {
        self.s(HORIZONTAL_SCROLL_HEIGHT).max(1)
    }

    fn horizontal_scroll_min_thumb(self) -> i32 {
        self.s(HORIZONTAL_SCROLL_MIN_THUMB).max(1)
    }

    fn resize_border(self) -> i32 {
        self.s(RESIZE_BORDER).max(6)
    }

    fn icon_scale(self) -> i32 {
        2
    }

    fn title_button(self, button: TitleButton) -> ControlRect {
        match button {
            TitleButton::Run => ControlRect::new(self.s(20), self.s(18), self.s(40), self.s(40)),
            TitleButton::Step => ControlRect::new(self.s(72), self.s(18), self.s(40), self.s(40)),
            TitleButton::Minimize => {
                ControlRect::new(self.width - self.s(104), self.s(18), self.s(40), self.s(40))
            }
            TitleButton::Close => {
                ControlRect::new(self.width - self.s(52), self.s(18), self.s(40), self.s(40))
            }
        }
    }

    fn hidden_value_width(self, left: i32) -> i32 {
        (self.value_right() - self.s(left)).max(1)
    }
}

#[derive(Clone, Copy)]
struct HorizontalScrollMetrics {
    max_scroll: i32,
    viewport_width: i32,
    track: RECT,
    thumb: RECT,
}

#[derive(Clone, Copy)]
struct HorizontalScrollDrag {
    start_x: i32,
    start_scroll: i32,
}

#[derive(Clone, Copy)]
struct CodeText<'a> {
    src: &'a str,
    out: &'a str,
    language: &'a str,
    status: &'a str,
    diagnostic: &'a str,
    hover_target: Option<HoverTarget>,
    blink_on: bool,
    spinner: usize,
    update_available: bool,
}

#[derive(Clone, Copy)]
struct CodeRender {
    layout: WindowLayout,
    scroll_x: i32,
}

#[derive(Clone, Copy)]
enum SelectableFont {
    Title,
    Code,
}

struct SelectableLine {
    text: String,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    scrollable: bool,
    font: SelectableFont,
}

enum GuiEvent {
    Status(String),
    Error(String),
    UpdateAvailable(ReleaseInfo),
    UpdateUnavailable(ReleaseInfo),
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

    update_window_region(hwnd);

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
            status_text: String::new(),
            diagnostic_text: String::new(),
            config,
            config_path,
            startup_error,
            watcher: None,
            tx,
            rx,
            active: false,
            spinner: 0,
            hover_target: None,
            pressed_title_button: None,
            text_selection: None,
            text_drag: None,
            horizontal_scroll: 0,
            horizontal_scroll_metrics: None,
            horizontal_scroll_drag: None,
            update_check_started: false,
            update_busy: false,
            update: None,
        }
    }
}

impl WatchHandle {
    fn matches(&self, src_dir: &Path, out_dir: &Path, language: LanguageMode) -> bool {
        self.src_dir == src_dir && self.out_dir == out_dir && self.language == language
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
            layout_controls(hwnd);
            SetTimer(hwnd, TIMER_ID, TIMER_INTERVAL_MS, None);
            start_update_check(hwnd);
            0
        }
        WM_SIZE => {
            handle_window_size(hwnd);
            0
        }
        WM_GETMINMAXINFO => {
            set_min_window_size(lparam);
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as i32;
            match id {
                ID_RUN => toggle_run(hwnd),
                ID_STEP => sync_once(hwnd),
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
        WM_MOUSEMOVE => {
            let point = point_from_lparam(lparam);
            if handle_text_selection_mouse_move(hwnd, point) {
                return 0;
            }
            if !handle_horizontal_scroll_mouse_move(hwnd, point) {
                update_hover_from_point(hwnd, point);
            }
            0
        }
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            let point = point_from_lparam(lparam);
            if handle_title_button_mouse_down(hwnd, point) {
                return 0;
            }
            if !handle_horizontal_scroll_mouse_down(hwnd, point) {
                handle_text_selection_mouse_down(hwnd, point);
            }
            0
        }
        WM_LBUTTONUP => {
            let point = point_from_lparam(lparam);
            if !finish_title_button_press(hwnd, point) && !finish_text_selection_drag(hwnd, point) {
                finish_horizontal_scroll_drag(hwnd);
            }
            0
        }
        WM_KEYDOWN => {
            if handle_key_down(hwnd, wparam) {
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_NCHITTEST => hit_test(hwnd, lparam),
        WM_ERASEBKGND => 1,
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => color_control(msg, wparam, lparam),
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
    let layout = layout_for_hwnd(hwnd);
    create_control(
        hwnd,
        "STATIC",
        "src_dir =",
        WS_CHILD,
        ControlRect::new(
            layout.s(CONTENT_LEFT),
            layout.s(SRC_ROW_TOP),
            layout.s(96),
            layout.text_row_height(),
        ),
        ID_SRC_LABEL,
    );
    create_control(
        hwnd,
        "STATIC",
        "language =",
        WS_CHILD,
        ControlRect::new(
            layout.s(CONTENT_LEFT),
            layout.s(LANGUAGE_ROW_TOP),
            layout.s(110),
            layout.text_row_height(),
        ),
        ID_LANGUAGE_LABEL,
    );
    let src_edit = create_control(
        hwnd,
        "EDIT",
        "",
        WS_CHILD | ES_AUTOHSCROLL as u32,
        ControlRect::new(
            layout.s(PATH_VALUE_LEFT),
            layout.s(SRC_ROW_TOP),
            layout.hidden_value_width(PATH_VALUE_LEFT),
            layout.s(30),
        ),
        ID_SRC_EDIT,
    );

    create_control(
        hwnd,
        "STATIC",
        "out_dir =",
        WS_CHILD,
        ControlRect::new(
            layout.s(CONTENT_LEFT),
            layout.s(OUT_ROW_TOP),
            layout.s(96),
            layout.text_row_height(),
        ),
        ID_OUT_LABEL,
    );
    let out_edit = create_control(
        hwnd,
        "EDIT",
        "",
        WS_CHILD | ES_AUTOHSCROLL as u32,
        ControlRect::new(
            layout.s(PATH_VALUE_LEFT),
            layout.s(OUT_ROW_TOP),
            layout.hidden_value_width(PATH_VALUE_LEFT),
            layout.s(30),
        ),
        ID_OUT_EDIT,
    );

    set_window_text(src_edit, &state.config.src_dir);
    set_window_text(out_edit, &state.config.out_dir);
    if let Some(error) = &state.startup_error {
        set_diagnostic(hwnd, error);
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

unsafe fn layout_for_hwnd(hwnd: HWND) -> WindowLayout {
    let mut rect: RECT = std::mem::zeroed();
    if GetClientRect(hwnd, &mut rect) == 0 {
        return WindowLayout::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    }
    WindowLayout::new(rect.right - rect.left, rect.bottom - rect.top)
}

unsafe fn layout_controls(hwnd: HWND) {
    let layout = layout_for_hwnd(hwnd);
    move_control(
        hwnd,
        ID_SRC_EDIT,
        ControlRect::new(
            layout.s(PATH_VALUE_LEFT),
            layout.s(SRC_ROW_TOP),
            layout.hidden_value_width(PATH_VALUE_LEFT),
            layout.s(30),
        ),
    );
    move_control(
        hwnd,
        ID_OUT_EDIT,
        ControlRect::new(
            layout.s(PATH_VALUE_LEFT),
            layout.s(OUT_ROW_TOP),
            layout.hidden_value_width(PATH_VALUE_LEFT),
            layout.s(30),
        ),
    );
}

unsafe fn handle_window_size(hwnd: HWND) {
    update_window_region(hwnd);
    layout_controls(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn move_control(parent: HWND, id: i32, bounds: ControlRect) {
    let control = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id);
    if !control.is_null() {
        MoveWindow(control, bounds.x, bounds.y, bounds.width, bounds.height, 0);
    }
}

unsafe fn update_window_region(hwnd: HWND) {
    let layout = layout_for_hwnd(hwnd);
    let radius = layout.s(10).max(1);
    let rounded = CreateRoundRectRgn(0, 0, layout.width + 1, layout.height + 1, radius, radius);
    if !rounded.is_null() {
        SetWindowRgn(hwnd, rounded, 0);
    }
}

unsafe fn set_min_window_size(lparam: LPARAM) {
    let info = &mut *(lparam as *mut MINMAXINFO);
    info.ptMinTrackSize.x = MIN_WINDOW_WIDTH;
    info.ptMinTrackSize.y = MIN_WINDOW_HEIGHT;
}

unsafe fn paint_window(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let screen_hdc = BeginPaint(hwnd, &mut ps);
    let layout = layout_for_hwnd(hwnd);
    let buffer_dc = CreateCompatibleDC(screen_hdc);
    let buffer_bitmap = if !buffer_dc.is_null() {
        CreateCompatibleBitmap(screen_hdc, layout.width, layout.height)
    } else {
        null_mut()
    };
    let old_buffer_bitmap = if !buffer_dc.is_null() && !buffer_bitmap.is_null() {
        SelectObject(buffer_dc, buffer_bitmap as _)
    } else {
        null_mut()
    };
    let hdc = if !buffer_dc.is_null() && !buffer_bitmap.is_null() {
        buffer_dc
    } else {
        screen_hdc
    };
    let (
        src_text,
        out_text,
        language_text,
        status_text,
        diagnostic_text,
        hover_target,
        blink_on,
        spinner,
        update_available,
        requested_scroll,
        active,
        pressed_title_button,
        text_selection,
    ) = state_from_hwnd(hwnd)
        .map(|state| {
            (
                state.config.src_dir.clone(),
                state.config.out_dir.clone(),
                state.config.language.display_name().to_string(),
                state.status_text.clone(),
                state.diagnostic_text.clone(),
                state.hover_target,
                state.spinner % 4 < 2,
                state.spinner,
                update_clickable(state),
                state.horizontal_scroll,
                state.active,
                state.pressed_title_button,
                state.text_selection,
            )
        })
        .unwrap_or_else(|| {
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                None,
                true,
                0,
                false,
                0,
                false,
                None,
                None,
            )
        });
    let mut rendered_scroll_metrics = None;
    let mut rendered_scroll = 0;

    with_theme(|theme| {
        let paint_state = SaveDC(hdc);
        let code_text = CodeText {
            src: &src_text,
            out: &out_text,
            language: &language_text,
            status: &status_text,
            diagnostic: &diagnostic_text,
            hover_target,
            blink_on,
            spinner,
            update_available,
        };
        let virtual_width = measure_code_content_width(hdc, theme, code_text, layout);
        rendered_scroll_metrics =
            horizontal_scroll_metrics(layout, virtual_width, requested_scroll.max(0));
        rendered_scroll = rendered_scroll_metrics
            .map(|metrics| requested_scroll.clamp(0, metrics.max_scroll))
            .unwrap_or(0);
        let render = CodeRender {
            layout,
            scroll_x: rendered_scroll,
        };

        let background = RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.height,
        };
        FillRect(hdc, &background, theme.panel);
        SelectObject(hdc, theme.no_outline as _);
        SelectObject(hdc, theme.panel as _);
        let window_radius = layout.s(10).max(1);
        RoundRect(
            hdc,
            0,
            0,
            layout.width,
            layout.height,
            window_radius,
            window_radius,
        );

        let title = RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.title_height(),
        };
        FillRect(hdc, &title, theme.title);
        draw_title_buttons(hdc, theme, layout, active, pressed_title_button);

        let editor = RECT {
            left: layout.editor_left(),
            top: layout.editor_top(),
            right: layout.editor_right(),
            bottom: layout.editor_bottom(),
        };
        SelectObject(hdc, theme.no_outline as _);
        SelectObject(hdc, theme.edit as _);
        RoundRect(
            hdc,
            editor.left,
            editor.top,
            editor.right,
            editor.bottom,
            layout.s(8).max(1),
            layout.s(8).max(1),
        );

        let gutter_line = RECT {
            left: layout.gutter_right() - layout.s(1),
            top: layout.editor_top(),
            right: layout.gutter_right() + layout.s(2),
            bottom: layout.editor_bottom(),
        };
        FillRect(hdc, &gutter_line, theme.gutter_line);

        if needs_scrollbar(&status_text) {
            let scrollbar = RECT {
                left: layout.editor_right() - layout.s(12),
                top: layout.editor_top() + layout.s(6),
                right: layout.editor_right() - layout.s(2),
                bottom: layout.editor_bottom() - layout.s(330),
            };
            FillRect(hdc, &scrollbar, theme.scroll);
        }

        draw_text_selection(hdc, theme, code_text, render, text_selection);
        draw_title_text(hdc, theme, layout, "Transplanter");
        let clip_state = SaveDC(hdc);
        IntersectClipRect(
            hdc,
            layout.content_left(),
            layout.editor_top(),
            layout.code_right(),
            layout.editor_bottom(),
        );
        draw_import_line(hdc, theme, render);
        draw_status_text(hdc, theme, code_text, render);
        draw_diagnostic_text(hdc, theme, code_text, render);
        draw_config_text(hdc, theme, code_text, render);
        RestoreDC(hdc, clip_state);

        if let Some(metrics) = rendered_scroll_metrics {
            draw_horizontal_scrollbar(hdc, theme, metrics);
        }
        RestoreDC(hdc, paint_state);
    });

    if hdc != screen_hdc {
        SelectClipRgn(screen_hdc, null_mut());
        BitBlt(
            screen_hdc,
            0,
            0,
            layout.width,
            layout.height,
            hdc,
            0,
            0,
            SRCCOPY,
        );
        if !old_buffer_bitmap.is_null() {
            SelectObject(buffer_dc, old_buffer_bitmap);
        }
        DeleteObject(buffer_bitmap as _);
        DeleteDC(buffer_dc);
    } else if !buffer_dc.is_null() {
        DeleteDC(buffer_dc);
    }

    EndPaint(hwnd, &ps);

    if let Some(state) = state_from_hwnd(hwnd) {
        state.horizontal_scroll = rendered_scroll;
        state.horizontal_scroll_metrics = rendered_scroll_metrics;
    }
}

unsafe fn draw_title_buttons(
    hdc: HDC,
    theme: &Theme,
    layout: WindowLayout,
    active: bool,
    pressed: Option<TitleButton>,
) {
    for button in [
        TitleButton::Run,
        TitleButton::Step,
        TitleButton::Minimize,
        TitleButton::Close,
    ] {
        draw_title_button(hdc, theme, layout, button, active, pressed == Some(button));
    }
}

unsafe fn draw_title_button(
    hdc: HDC,
    theme: &Theme,
    layout: WindowLayout,
    button: TitleButton,
    active: bool,
    pressed: bool,
) {
    let bounds = layout.title_button(button).to_rect();
    let active_controls = active && matches!(button, TitleButton::Run | TitleButton::Step);
    let surface = if active_controls {
        ButtonSurface {
            brush: if pressed {
                theme.run_active_down
            } else {
                theme.run_active
            },
            background: ButtonBackground::Title,
        }
    } else {
        ButtonSurface {
            brush: if pressed {
                theme.button_down
            } else {
                theme.button
            },
            background: ButtonBackground::Title,
        }
    };
    let face_rect = draw_game_button_surface(hdc, &bounds, theme, surface);

    match button {
        TitleButton::Run => {
            let icon = if active { &STOP_ICON } else { &RUN_ICON };
            draw_mask_icon(hdc, &face_rect, icon, layout.icon_scale(), theme.icon);
        }
        TitleButton::Step => {
            let icon = if active { &PAUSE_ICON } else { &STEP_ICON };
            draw_mask_icon(hdc, &face_rect, icon, layout.icon_scale(), theme.icon);
        }
        TitleButton::Minimize => {
            draw_mask_icon(
                hdc,
                &face_rect,
                &MINIMIZE_ICON,
                layout.icon_scale(),
                theme.icon,
            );
        }
        TitleButton::Close => {
            let icon_background = if pressed {
                COLOR_BUTTON_DOWN
            } else {
                COLOR_BUTTON
            };
            draw_close_icon(hdc, &face_rect, icon_background);
        }
    }
}

unsafe fn draw_title_text(hdc: HDC, theme: &Theme, layout: WindowLayout, text: &str) {
    let mut rect = title_text_rect(layout);
    if !rect_has_area(&rect) {
        return;
    }

    let text = wide(text);
    SelectObject(hdc, theme.title_font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, COLOR_TEXT);
    DrawTextW(
        hdc,
        text.as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );
}

fn title_text_rect(layout: WindowLayout) -> RECT {
    RECT {
        left: layout.s(124),
        top: layout.s(20),
        right: layout.width - layout.s(120),
        bottom: layout.s(56),
    }
}

unsafe fn draw_text_selection(
    hdc: HDC,
    theme: &Theme,
    text: CodeText<'_>,
    render: CodeRender,
    selection: Option<TextSelection>,
) {
    let Some(selection) = selection else {
        return;
    };
    if selection_is_empty(selection) {
        return;
    }

    let lines = selectable_lines(text, render.layout);
    let Some((start, end)) = normalized_selection(selection, &lines) else {
        return;
    };

    for (line_index, line) in lines.iter().enumerate() {
        if line_index < start.line || line_index > end.line {
            continue;
        }

        let line_chars = line.text.chars().count();
        let start_char = if line_index == start.line {
            start.char_index.min(line_chars)
        } else {
            0
        };
        let end_char = if line_index == end.line {
            end.char_index.min(line_chars)
        } else {
            line_chars
        };
        if start_char >= end_char {
            continue;
        }

        let font = selectable_font(theme, line.font);
        let scroll = if line.scrollable { render.scroll_x } else { 0 };
        let text_left = line.left - scroll;
        let mut selection_rect = RECT {
            left: text_left
                + measure_text_prefix_width(hdc, font, render.layout, &line.text, start_char),
            top: line.top + render.layout.s(3),
            right: text_left
                + measure_text_prefix_width(hdc, font, render.layout, &line.text, end_char),
            bottom: line.bottom - render.layout.s(3),
        };
        selection_rect.left = selection_rect.left.clamp(line.left, line.right);
        selection_rect.right = selection_rect.right.clamp(line.left, line.right);
        if rect_has_area(&selection_rect) {
            FillRect(hdc, &selection_rect, theme.selection);
        }
    }
}

fn selectable_lines(text: CodeText<'_>, layout: WindowLayout) -> Vec<SelectableLine> {
    let mut lines = Vec::new();
    let title = title_text_rect(layout);
    lines.push(SelectableLine {
        text: "Transplanter".to_string(),
        left: title.left,
        top: title.top,
        right: title.right,
        bottom: title.bottom,
        scrollable: false,
        font: SelectableFont::Title,
    });

    push_code_line(
        &mut lines,
        layout,
        IMPORT_ROW_TOP,
        format!("import {} as transplanter", transplanter_version_module()),
    );
    if let Some(status) = status_display_text(text.status, text.update_available, text.spinner) {
        push_code_line(&mut lines, layout, STATUS_ROW_TOP, status);
    }
    if let Some(diagnostic) = diagnostic_display_text(text.diagnostic) {
        push_code_line(&mut lines, layout, DIAGNOSTIC_ROW_TOP, diagnostic);
    }
    push_code_line(
        &mut lines,
        layout,
        SRC_ROW_TOP,
        format!(
            "src_dir  = {}",
            selectable_value_text(text.src, text.blink_on)
        ),
    );
    push_code_line(
        &mut lines,
        layout,
        OUT_ROW_TOP,
        format!(
            "out_dir  = {}",
            selectable_value_text(text.out, text.blink_on)
        ),
    );
    push_code_line(
        &mut lines,
        layout,
        LANGUAGE_ROW_TOP,
        format!(
            "language = {}",
            selectable_value_text(text.language, text.blink_on)
        ),
    );
    push_code_line(
        &mut lines,
        layout,
        CALL_ROW_TOP,
        "transplanter(src_dir, out_dir, language)".to_string(),
    );
    lines
}

fn push_code_line(lines: &mut Vec<SelectableLine>, layout: WindowLayout, top: i32, text: String) {
    let top = layout.s(top);
    lines.push(SelectableLine {
        text,
        left: layout.content_left(),
        top,
        right: layout.code_right(),
        bottom: top + layout.text_row_height(),
        scrollable: true,
        font: SelectableFont::Code,
    });
}

fn selectable_value_text(value: &str, blink_on: bool) -> &str {
    if value.trim().is_empty() {
        if blink_on { "_" } else { "" }
    } else {
        value
    }
}

fn selectable_font(theme: &Theme, font: SelectableFont) -> HFONT {
    match font {
        SelectableFont::Title => theme.title_font,
        SelectableFont::Code => theme.code_font,
    }
}

unsafe fn draw_config_text(hdc: HDC, theme: &Theme, text: CodeText<'_>, render: CodeRender) {
    draw_assignment_line(
        hdc,
        theme,
        render,
        AssignmentLine::src("src_dir", text.src, text.blink_on, text.hover_target),
    );
    draw_assignment_line(
        hdc,
        theme,
        render,
        AssignmentLine::out("out_dir", text.out, text.blink_on, text.hover_target),
    );
    draw_assignment_line(
        hdc,
        theme,
        render,
        AssignmentLine::language("language", text.language, text.blink_on, text.hover_target),
    );
    draw_transplanter_call(hdc, theme, render);
}

unsafe fn draw_import_line(hdc: HDC, theme: &Theme, render: CodeRender) {
    let module_name = transplanter_version_module();
    let module_name = format!(" {module_name}");
    draw_code_segments(
        hdc,
        theme,
        render,
        IMPORT_ROW_TOP,
        &[
            ("import", COLOR_KEYWORD),
            (&module_name, COLOR_BUILTIN),
            (" as", COLOR_KEYWORD),
            (" transplanter", COLOR_BUILTIN),
        ],
    );
}

unsafe fn draw_transplanter_call(hdc: HDC, theme: &Theme, render: CodeRender) {
    draw_code_segments(
        hdc,
        theme,
        render,
        CALL_ROW_TOP,
        &[
            ("transplanter", COLOR_BUILTIN),
            ("(", COLOR_TEXT),
            ("src_dir", COLOR_TEXT),
            (", ", COLOR_TEXT),
            ("out_dir", COLOR_TEXT),
            (", ", COLOR_TEXT),
            ("language", COLOR_TEXT),
            (")", COLOR_TEXT),
        ],
    );
}

unsafe fn draw_code_segments(
    hdc: HDC,
    theme: &Theme,
    render: CodeRender,
    top: i32,
    segments: &[(&str, u32)],
) {
    SelectObject(hdc, theme.code_font as _);
    SetBkMode(hdc, TRANSPARENT as i32);

    let top = render.layout.s(top);
    let row_height = render.layout.text_row_height();
    let mut left = render.layout.content_left();
    for (text, color) in segments {
        let text = wide(text);
        let mut rect = RECT {
            left: left - render.scroll_x,
            top,
            right: render.layout.code_right(),
            bottom: top + row_height,
        };
        if rect_has_area(&rect) {
            SetTextColor(hdc, *color);
            DrawTextW(
                hdc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );
        }

        let mut measure = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: row_height,
        };
        DrawTextW(
            hdc,
            text.as_ptr(),
            -1,
            &mut measure,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_CALCRECT,
        );
        left += measure.right - measure.left;
    }
}

fn transplanter_version_module() -> String {
    let version = env!("CARGO_PKG_VERSION")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("transplanter_v{version}")
}

struct AssignmentLine<'a> {
    key: &'a str,
    value: &'a str,
    top: i32,
    equal_left: i32,
    value_left: i32,
    value_color: u32,
    hovered: bool,
    blink_on: bool,
}

impl<'a> AssignmentLine<'a> {
    fn src(key: &'a str, value: &'a str, blink_on: bool, hover: Option<HoverTarget>) -> Self {
        Self {
            key,
            value,
            top: SRC_ROW_TOP,
            equal_left: PATH_EQUAL_LEFT,
            value_left: PATH_VALUE_LEFT,
            value_color: COLOR_TEXT,
            hovered: hover == Some(HoverTarget::SrcDir),
            blink_on,
        }
    }

    fn out(key: &'a str, value: &'a str, blink_on: bool, hover: Option<HoverTarget>) -> Self {
        Self {
            key,
            value,
            top: OUT_ROW_TOP,
            equal_left: PATH_EQUAL_LEFT,
            value_left: PATH_VALUE_LEFT,
            value_color: COLOR_TEXT,
            hovered: hover == Some(HoverTarget::OutDir),
            blink_on,
        }
    }

    fn language(key: &'a str, value: &'a str, blink_on: bool, hover: Option<HoverTarget>) -> Self {
        Self {
            key,
            value,
            top: LANGUAGE_ROW_TOP,
            equal_left: LANGUAGE_EQUAL_LEFT,
            value_left: LANGUAGE_VALUE_LEFT,
            value_color: COLOR_BUILTIN,
            hovered: hover == Some(HoverTarget::Language),
            blink_on,
        }
    }
}

unsafe fn draw_assignment_line(
    hdc: HDC,
    theme: &Theme,
    render: CodeRender,
    line: AssignmentLine<'_>,
) {
    SelectObject(hdc, theme.code_font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    let top = render.layout.s(line.top);
    let row_height = render.layout.text_row_height();
    let content_left = render.layout.content_left();
    let equal_left = render.layout.s(line.equal_left);
    let value_left = render.layout.s(line.value_left);

    let mut key_rect = RECT {
        left: content_left - render.scroll_x,
        top,
        right: equal_left - render.layout.s(4) - render.scroll_x,
        bottom: top + row_height,
    };
    let key = wide(line.key);
    if rect_has_area(&key_rect) {
        SetTextColor(hdc, COLOR_KEYWORD);
        DrawTextW(
            hdc,
            key.as_ptr(),
            -1,
            &mut key_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }

    let mut equal_rect = RECT {
        left: equal_left - render.scroll_x,
        top,
        right: value_left - render.scroll_x,
        bottom: top + row_height,
    };
    let equal = wide("=");
    if rect_has_area(&equal_rect) {
        SetTextColor(hdc, COLOR_KEYWORD);
        DrawTextW(
            hdc,
            equal.as_ptr(),
            -1,
            &mut equal_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }

    let display_value = if line.value.trim().is_empty() {
        if line.blink_on { "_" } else { "" }
    } else {
        line.value
    };
    let mut value_rect = RECT {
        left: value_left - render.scroll_x,
        top,
        right: render.layout.code_right(),
        bottom: top + row_height,
    };
    let value = wide(display_value);
    SelectObject(
        hdc,
        if line.hovered {
            theme.code_hover_font
        } else {
            theme.code_font
        } as _,
    );
    SetTextColor(hdc, line.value_color);
    if rect_has_area(&value_rect) {
        DrawTextW(
            hdc,
            value.as_ptr(),
            -1,
            &mut value_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }
}

unsafe fn draw_status_text(hdc: HDC, theme: &Theme, text: CodeText<'_>, render: CodeRender) {
    let Some(compact) = status_display_value(text.status, text.update_available, text.spinner)
    else {
        return;
    };
    let top = render.layout.s(STATUS_ROW_TOP);
    let mut rect = RECT {
        left: render.layout.content_left() - render.scroll_x,
        top,
        right: render.layout.code_right(),
        bottom: top + render.layout.text_row_height(),
    };
    if !rect_has_area(&rect) {
        return;
    }

    let display_text = wide(&format!("status = \"{compact}\""));
    SelectObject(
        hdc,
        if text.hover_target == Some(HoverTarget::UpdateStatus) {
            theme.code_hover_font
        } else {
            theme.code_font
        } as _,
    );
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(
        hdc,
        if compact.starts_with("エラー:") {
            COLOR_KEYWORD
        } else if text.update_available && text.blink_on {
            COLOR_TEXT
        } else {
            COLOR_BUILTIN
        },
    );
    DrawTextW(
        hdc,
        display_text.as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );
}

unsafe fn draw_diagnostic_text(hdc: HDC, theme: &Theme, text: CodeText<'_>, render: CodeRender) {
    let Some(diagnostic) = diagnostic_display_value(text.diagnostic) else {
        return;
    };

    draw_code_segments(
        hdc,
        theme,
        render,
        DIAGNOSTIC_ROW_TOP,
        &[
            ("error", COLOR_KEYWORD),
            (" = ", COLOR_KEYWORD),
            ("\"", COLOR_TEXT),
            (&diagnostic, COLOR_TEXT),
            ("\"", COLOR_TEXT),
        ],
    );
}

fn truncate_for_status(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut shortened = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

fn needs_scrollbar(status: &str) -> bool {
    status.lines().count() > 6 || status.chars().count() > 180
}

unsafe fn measure_code_content_width(
    hdc: HDC,
    theme: &Theme,
    text: CodeText<'_>,
    layout: WindowLayout,
) -> i32 {
    let mut right = layout.content_left();
    let module_name = transplanter_version_module();
    let module_name = format!(" {module_name}");
    right = right.max(
        layout.content_left()
            + measure_segments_width(
                hdc,
                theme,
                layout,
                &[
                    ("import", COLOR_KEYWORD),
                    (&module_name, COLOR_BUILTIN),
                    (" as", COLOR_KEYWORD),
                    (" transplanter", COLOR_BUILTIN),
                ],
            ),
    );

    if let Some(status_text) = status_display_text(text.status, text.update_available, text.spinner)
    {
        right = right.max(
            layout.content_left() + measure_text_width(hdc, theme.code_font, layout, &status_text),
        );
    }

    if let Some(diagnostic_text) = diagnostic_display_text(text.diagnostic) {
        right = right.max(
            layout.content_left()
                + measure_text_width(hdc, theme.code_font, layout, &diagnostic_text),
        );
    }

    right = right.max(measure_assignment_right(
        hdc,
        theme,
        layout,
        PATH_VALUE_LEFT,
        text.src,
        text.blink_on,
    ));
    right = right.max(measure_assignment_right(
        hdc,
        theme,
        layout,
        PATH_VALUE_LEFT,
        text.out,
        text.blink_on,
    ));
    right = right.max(measure_assignment_right(
        hdc,
        theme,
        layout,
        LANGUAGE_VALUE_LEFT,
        text.language,
        text.blink_on,
    ));
    right = right.max(
        layout.content_left()
            + measure_segments_width(
                hdc,
                theme,
                layout,
                &[
                    ("transplanter", COLOR_BUILTIN),
                    ("(", COLOR_TEXT),
                    ("src_dir", COLOR_TEXT),
                    (", ", COLOR_TEXT),
                    ("out_dir", COLOR_TEXT),
                    (", ", COLOR_TEXT),
                    ("language", COLOR_TEXT),
                    (")", COLOR_TEXT),
                ],
            ),
    );

    right - layout.content_left()
}

unsafe fn measure_assignment_right(
    hdc: HDC,
    theme: &Theme,
    layout: WindowLayout,
    value_left: i32,
    value: &str,
    blink_on: bool,
) -> i32 {
    let display_value = if value.trim().is_empty() {
        if blink_on { "_" } else { "" }
    } else {
        value
    };
    layout.s(value_left) + measure_text_width(hdc, theme.code_font, layout, display_value)
}

unsafe fn measure_segments_width(
    hdc: HDC,
    theme: &Theme,
    layout: WindowLayout,
    segments: &[(&str, u32)],
) -> i32 {
    segments
        .iter()
        .map(|(text, _)| measure_text_width(hdc, theme.code_font, layout, text))
        .sum()
}

unsafe fn measure_text_width(hdc: HDC, font: HFONT, layout: WindowLayout, text: &str) -> i32 {
    let text = wide(text);
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: layout.text_row_height(),
    };
    SelectObject(hdc, font as _);
    DrawTextW(
        hdc,
        text.as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_CALCRECT,
    );
    rect.right - rect.left
}

fn status_display_text(status: &str, update_available: bool, spinner: usize) -> Option<String> {
    status_display_value(status, update_available, spinner)
        .map(|value| format!("status = \"{value}\""))
}

fn status_display_value(status: &str, update_available: bool, spinner: usize) -> Option<String> {
    let status = if update_available {
        STATUS_UPDATE_AVAILABLE.to_string()
    } else if status.trim().is_empty() {
        format!("{STATUS_CHECKING} {}", command_spinner(spinner))
    } else {
        status.to_string()
    };
    if status.trim().is_empty() {
        return None;
    }

    let compact = compact_error_message(&status);
    let compact = truncate_for_status(&compact, 42);
    Some(compact)
}

fn command_spinner(spinner: usize) -> char {
    ['/', '-', '\\', '|'][spinner % 4]
}

fn diagnostic_display_text(diagnostic: &str) -> Option<String> {
    diagnostic_display_value(diagnostic).map(|value| format!("error = \"{value}\""))
}

fn diagnostic_display_value(diagnostic: &str) -> Option<String> {
    let compact = compact_error_message(diagnostic);
    let compact = compact.trim();
    if compact.is_empty() {
        return None;
    }

    Some(code_string_literal_value(compact))
}

fn code_string_literal_value(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_space = false;
    for ch in value.chars() {
        let ch = match ch {
            '\r' | '\n' | '\t' => ' ',
            '"' => '\'',
            _ => ch,
        };
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
            }
            last_was_space = true;
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output
}

fn horizontal_scroll_metrics(
    layout: WindowLayout,
    virtual_width: i32,
    requested_scroll: i32,
) -> Option<HorizontalScrollMetrics> {
    let viewport_width = (layout.code_right() - layout.content_left()).max(1);
    let max_scroll = (virtual_width - viewport_width).max(0);
    if max_scroll <= 0 {
        return None;
    }

    let track = RECT {
        left: layout.content_left(),
        top: layout.editor_bottom() - layout.horizontal_scroll_height() - layout.s(5),
        right: layout.code_right(),
        bottom: layout.editor_bottom() - layout.s(5),
    };
    let track_width = (track.right - track.left).max(1);
    let thumb_width = ((viewport_width * track_width) / virtual_width.max(1))
        .clamp(layout.horizontal_scroll_min_thumb(), track_width);
    let travel = (track_width - thumb_width).max(1);
    let scroll = requested_scroll.clamp(0, max_scroll);
    let thumb_left = track.left + (travel * scroll) / max_scroll.max(1);
    let thumb = RECT {
        left: thumb_left,
        top: track.top,
        right: thumb_left + thumb_width,
        bottom: track.bottom,
    };

    Some(HorizontalScrollMetrics {
        max_scroll,
        viewport_width,
        track,
        thumb,
    })
}

unsafe fn draw_horizontal_scrollbar(hdc: HDC, theme: &Theme, metrics: HorizontalScrollMetrics) {
    if rect_has_area(&metrics.track) {
        FillRect(hdc, &metrics.track, theme.edit_shadow_deep);
    }
    if rect_has_area(&metrics.thumb) {
        FillRect(hdc, &metrics.thumb, theme.scroll);
    }
}

fn title_button_at(point: POINT, layout: WindowLayout) -> Option<TitleButton> {
    [
        TitleButton::Run,
        TitleButton::Step,
        TitleButton::Minimize,
        TitleButton::Close,
    ]
    .into_iter()
    .find(|button| point_in_rect(point, layout.title_button(*button).to_rect()))
}

unsafe fn hit_test(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let mut point = point_from_lparam(lparam);
    ScreenToClient(hwnd, &mut point);
    let layout = layout_for_hwnd(hwnd);
    let resize_border = layout.resize_border();
    let left = point.x >= 0 && point.x < resize_border;
    let right = point.x >= layout.width - resize_border && point.x < layout.width;
    let top = point.y >= 0 && point.y < resize_border;
    let bottom = point.y >= layout.height - resize_border && point.y < layout.height;

    match (left, right, top, bottom) {
        (true, _, true, _) => HTTOPLEFT as LRESULT,
        (_, true, true, _) => HTTOPRIGHT as LRESULT,
        (true, _, _, true) => HTBOTTOMLEFT as LRESULT,
        (_, true, _, true) => HTBOTTOMRIGHT as LRESULT,
        (true, _, _, _) => HTLEFT as LRESULT,
        (_, true, _, _) => HTRIGHT as LRESULT,
        (_, _, true, _) => HTTOP as LRESULT,
        (_, _, _, true) => HTBOTTOM as LRESULT,
        _ if title_button_at(point, layout).is_some() => HTCLIENT as LRESULT,
        _ if point_in_rect(point, title_text_rect(layout)) => HTCLIENT as LRESULT,
        _ if point.y >= 0
            && point.y < layout.title_height()
            && point.x > layout.s(120)
            && point.x < layout.width - layout.s(112) =>
        {
            HTCAPTION as LRESULT
        }
        _ => HTCLIENT as LRESULT,
    }
}

unsafe fn handle_title_button_mouse_down(hwnd: HWND, point: POINT) -> bool {
    let layout = layout_for_hwnd(hwnd);
    let Some(button) = title_button_at(point, layout) else {
        return false;
    };
    if let Some(state) = state_from_hwnd(hwnd) {
        state.pressed_title_button = Some(button);
        SetCapture(hwnd);
        InvalidateRect(hwnd, null(), 0);
        return true;
    }
    false
}

unsafe fn finish_title_button_press(hwnd: HWND, point: POINT) -> bool {
    let pressed = if let Some(state) = state_from_hwnd(hwnd) {
        state.pressed_title_button.take()
    } else {
        None
    };
    let Some(button) = pressed else {
        return false;
    };

    ReleaseCapture();
    InvalidateRect(hwnd, null(), 0);
    if title_button_at(point, layout_for_hwnd(hwnd)) == Some(button) {
        activate_title_button(hwnd, button);
    }
    true
}

unsafe fn activate_title_button(hwnd: HWND, button: TitleButton) {
    match button {
        TitleButton::Run => toggle_run(hwnd),
        TitleButton::Step => sync_once(hwnd),
        TitleButton::Minimize => {
            ShowWindow(hwnd, SW_MINIMIZE);
        }
        TitleButton::Close => {
            DestroyWindow(hwnd);
        }
    }
}

unsafe fn handle_horizontal_scroll_mouse_down(hwnd: HWND, point: POINT) -> bool {
    let Some(state) = state_from_hwnd(hwnd) else {
        return false;
    };
    let Some(metrics) = state.horizontal_scroll_metrics else {
        return false;
    };

    if point_in_rect(point, metrics.thumb) {
        state.horizontal_scroll_drag = Some(HorizontalScrollDrag {
            start_x: point.x,
            start_scroll: state.horizontal_scroll,
        });
        SetCapture(hwnd);
        return true;
    }

    if point_in_rect(point, metrics.track) {
        let direction = if point.x < metrics.thumb.left { -1 } else { 1 };
        state.horizontal_scroll = (state.horizontal_scroll + direction * metrics.viewport_width)
            .clamp(0, metrics.max_scroll);
        InvalidateRect(hwnd, null(), 0);
        return true;
    }

    false
}

unsafe fn handle_horizontal_scroll_mouse_move(hwnd: HWND, point: POINT) -> bool {
    let Some(state) = state_from_hwnd(hwnd) else {
        return false;
    };
    let Some(drag) = state.horizontal_scroll_drag else {
        return false;
    };
    let Some(metrics) = state.horizontal_scroll_metrics else {
        return false;
    };

    let track_width = metrics.track.right - metrics.track.left;
    let thumb_width = metrics.thumb.right - metrics.thumb.left;
    let travel = (track_width - thumb_width).max(1);
    let delta = point.x - drag.start_x;
    state.horizontal_scroll =
        (drag.start_scroll + (delta * metrics.max_scroll) / travel).clamp(0, metrics.max_scroll);
    InvalidateRect(hwnd, null(), 0);
    true
}

unsafe fn finish_horizontal_scroll_drag(hwnd: HWND) {
    if let Some(state) = state_from_hwnd(hwnd)
        && state.horizontal_scroll_drag.take().is_some()
    {
        ReleaseCapture();
        InvalidateRect(hwnd, null(), 0);
    }
}

unsafe fn handle_text_selection_mouse_down(hwnd: HWND, point: POINT) -> bool {
    let Some(position) = text_position_at_point(hwnd, point, false) else {
        handle_text_click(hwnd, point);
        return false;
    };
    let layout = layout_for_hwnd(hwnd);
    let pending_target =
        state_from_hwnd(hwnd).and_then(|state| hit_target_at(point, state, layout));
    if let Some(state) = state_from_hwnd(hwnd) {
        state.text_selection = Some(TextSelection {
            anchor: position,
            active: position,
        });
        state.text_drag = Some(TextDrag {
            start_point: point,
            moved: false,
            pending_target,
        });
        SetCapture(hwnd);
        InvalidateRect(hwnd, null(), 0);
        return true;
    }
    false
}

unsafe fn handle_text_selection_mouse_move(hwnd: HWND, point: POINT) -> bool {
    let Some(drag) = state_from_hwnd(hwnd).and_then(|state| state.text_drag) else {
        return false;
    };
    let Some(position) = text_position_at_point(hwnd, point, true) else {
        return true;
    };
    if let Some(state) = state_from_hwnd(hwnd) {
        if let Some(selection) = &mut state.text_selection {
            selection.active = position;
        }
        if !state.text_drag.is_some_and(|drag| drag.moved)
            && point_distance_exceeds(drag.start_point, point, 3)
            && let Some(text_drag) = &mut state.text_drag
        {
            text_drag.moved = true;
        }
        InvalidateRect(hwnd, null(), 0);
        return true;
    }
    false
}

unsafe fn finish_text_selection_drag(hwnd: HWND, point: POINT) -> bool {
    let drag = if let Some(state) = state_from_hwnd(hwnd) {
        state.text_drag.take()
    } else {
        None
    };
    let Some(drag) = drag else {
        return false;
    };

    ReleaseCapture();
    if let Some(position) = text_position_at_point(hwnd, point, true)
        && let Some(state) = state_from_hwnd(hwnd)
        && let Some(selection) = &mut state.text_selection
    {
        selection.active = position;
    }

    let mut activate = None;
    if let Some(state) = state_from_hwnd(hwnd)
        && (!drag.moved || state.text_selection.is_some_and(selection_is_empty))
    {
        state.text_selection = None;
        activate = drag.pending_target;
    }
    InvalidateRect(hwnd, null(), 0);
    if let Some(target) = activate {
        activate_text_target(hwnd, target);
    }
    true
}

fn point_distance_exceeds(start: POINT, current: POINT, threshold: i32) -> bool {
    (start.x - current.x).abs() > threshold || (start.y - current.y).abs() > threshold
}

unsafe fn handle_text_click(hwnd: HWND, point: POINT) {
    let layout = layout_for_hwnd(hwnd);
    let target = state_from_hwnd(hwnd).and_then(|state| hit_target_at(point, state, layout));
    if let Some(target) = target {
        activate_text_target(hwnd, target);
    }
}

unsafe fn activate_text_target(hwnd: HWND, target: HoverTarget) {
    match target {
        HoverTarget::SrcDir => browse_and_set_path(hwnd, ID_SRC_EDIT, true),
        HoverTarget::OutDir => browse_and_set_path(hwnd, ID_OUT_EDIT, false),
        HoverTarget::Language => cycle_language_mode(hwnd),
        HoverTarget::UpdateStatus => handle_update_clicked(hwnd),
    }
}

unsafe fn update_hover_from_cursor(hwnd: HWND) {
    let mut point: POINT = std::mem::zeroed();
    if GetCursorPos(&mut point) != 0 {
        ScreenToClient(hwnd, &mut point);
        update_hover_from_point(hwnd, point);
    }
}

unsafe fn update_hover_from_point(hwnd: HWND, point: POINT) {
    let layout = layout_for_hwnd(hwnd);
    let target = state_from_hwnd(hwnd).and_then(|state| hit_target_at(point, state, layout));
    if let Some(state) = state_from_hwnd(hwnd)
        && state.hover_target != target
    {
        state.hover_target = target;
        InvalidateRect(hwnd, null(), 0);
    }
}

fn scrolled_rect(layout: WindowLayout, left: i32, top: i32, right: i32, scroll_x: i32) -> RECT {
    let top = layout.s(top);
    RECT {
        left: layout.s(left) - scroll_x,
        top,
        right,
        bottom: top + layout.text_row_height(),
    }
}

unsafe fn hit_target_at(
    point: POINT,
    state: &GuiState,
    layout: WindowLayout,
) -> Option<HoverTarget> {
    for target in [
        HoverTarget::SrcDir,
        HoverTarget::OutDir,
        HoverTarget::Language,
        HoverTarget::UpdateStatus,
    ] {
        if target == HoverTarget::UpdateStatus && !update_clickable(state) {
            continue;
        }
        if point_in_rect(
            point,
            interactive_rect(target, layout, state.horizontal_scroll),
        ) {
            return Some(target);
        }
    }

    None
}

fn interactive_rect(target: HoverTarget, layout: WindowLayout, scroll_x: i32) -> RECT {
    let code_right = layout.code_right();
    match target {
        HoverTarget::SrcDir => {
            scrolled_rect(layout, PATH_VALUE_LEFT, SRC_ROW_TOP, code_right, scroll_x)
        }
        HoverTarget::OutDir => {
            scrolled_rect(layout, PATH_VALUE_LEFT, OUT_ROW_TOP, code_right, scroll_x)
        }
        HoverTarget::Language => scrolled_rect(
            layout,
            LANGUAGE_VALUE_LEFT,
            LANGUAGE_ROW_TOP,
            code_right,
            scroll_x,
        ),
        HoverTarget::UpdateStatus => {
            scrolled_rect(layout, CONTENT_LEFT, STATUS_ROW_TOP, code_right, scroll_x)
        }
    }
}

unsafe fn text_position_at_point(
    hwnd: HWND,
    point: POINT,
    clamp_to_nearest_line: bool,
) -> Option<TextPosition> {
    let layout = layout_for_hwnd(hwnd);
    let (text, scroll_x) = state_from_hwnd(hwnd)
        .map(|state| (CodeTextSnapshot::from_state(state), state.horizontal_scroll))?;
    let lines = selectable_lines(text.as_code_text(), layout);
    let line_index = selectable_line_index_at_point(&lines, point, clamp_to_nearest_line)?;
    let line = &lines[line_index];
    let hdc = GetDC(hwnd);
    if hdc.is_null() {
        return Some(TextPosition {
            line: line_index,
            char_index: 0,
        });
    }
    let char_index = with_theme(|theme| {
        let font = selectable_font(theme, line.font);
        let scroll = if line.scrollable { scroll_x } else { 0 };
        char_index_at_x(hdc, font, layout, &line.text, line.left - scroll, point.x)
    });
    ReleaseDC(hwnd, hdc);
    Some(TextPosition {
        line: line_index,
        char_index,
    })
}

fn selectable_line_index_at_point(
    lines: &[SelectableLine],
    point: POINT,
    clamp_to_nearest_line: bool,
) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    if let Some((index, _)) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| point.y >= line.top && point.y < line.bottom)
        && (clamp_to_nearest_line || (point.x >= lines[index].left && point.x < lines[index].right))
    {
        return Some(index);
    }

    if !clamp_to_nearest_line {
        return None;
    }

    if point.y < lines[0].top {
        return Some(0);
    }
    lines
        .iter()
        .enumerate()
        .min_by_key(|(_, line)| {
            if point.y < line.top {
                line.top - point.y
            } else if point.y >= line.bottom {
                point.y - line.bottom
            } else {
                0
            }
        })
        .map(|(index, _)| index)
}

unsafe fn char_index_at_x(
    hdc: HDC,
    font: HFONT,
    layout: WindowLayout,
    text: &str,
    text_left: i32,
    x: i32,
) -> usize {
    let relative_x = x - text_left;
    if relative_x <= 0 {
        return 0;
    }

    let chars = text.chars().count();
    for index in 0..chars {
        let left = measure_text_prefix_width(hdc, font, layout, text, index);
        let right = measure_text_prefix_width(hdc, font, layout, text, index + 1);
        let midpoint = left + (right - left) / 2;
        if relative_x < midpoint {
            return index;
        }
    }
    chars
}

unsafe fn measure_text_prefix_width(
    hdc: HDC,
    font: HFONT,
    layout: WindowLayout,
    text: &str,
    char_count: usize,
) -> i32 {
    let prefix = text.chars().take(char_count).collect::<String>();
    measure_text_width(hdc, font, layout, &prefix)
}

#[derive(Clone)]
struct CodeTextSnapshot {
    src: String,
    out: String,
    language: String,
    status: String,
    diagnostic: String,
    hover_target: Option<HoverTarget>,
    blink_on: bool,
    spinner: usize,
    update_available: bool,
}

impl CodeTextSnapshot {
    fn from_state(state: &GuiState) -> Self {
        Self {
            src: state.config.src_dir.clone(),
            out: state.config.out_dir.clone(),
            language: state.config.language.display_name().to_string(),
            status: state.status_text.clone(),
            diagnostic: state.diagnostic_text.clone(),
            hover_target: state.hover_target,
            blink_on: state.spinner % 4 < 2,
            spinner: state.spinner,
            update_available: update_clickable(state),
        }
    }

    fn as_code_text(&self) -> CodeText<'_> {
        CodeText {
            src: &self.src,
            out: &self.out,
            language: &self.language,
            status: &self.status,
            diagnostic: &self.diagnostic,
            hover_target: self.hover_target,
            blink_on: self.blink_on,
            spinner: self.spinner,
            update_available: self.update_available,
        }
    }
}

fn selection_is_empty(selection: TextSelection) -> bool {
    selection.anchor == selection.active
}

fn normalized_selection(
    selection: TextSelection,
    lines: &[SelectableLine],
) -> Option<(TextPosition, TextPosition)> {
    if lines.is_empty() {
        return None;
    }

    let clamp = |position: TextPosition| {
        let line = position.line.min(lines.len().saturating_sub(1));
        let char_index = position.char_index.min(lines[line].text.chars().count());
        TextPosition { line, char_index }
    };
    let anchor = clamp(selection.anchor);
    let active = clamp(selection.active);
    Some(if anchor <= active {
        (anchor, active)
    } else {
        (active, anchor)
    })
}

unsafe fn handle_key_down(hwnd: HWND, wparam: WPARAM) -> bool {
    let ctrl_down = (GetKeyState(0x11) as u16 & 0x8000) != 0;
    if !ctrl_down {
        return false;
    }

    match wparam as u32 {
        key if key == b'C' as u32 => {
            if let Some(text) = selected_text(hwnd) {
                copy_to_clipboard(hwnd, &text);
            }
            true
        }
        key if key == b'A' as u32 => {
            select_all_text(hwnd);
            true
        }
        _ => false,
    }
}

unsafe fn select_all_text(hwnd: HWND) {
    let Some((lines, _scroll_x)) = selectable_lines_for_hwnd(hwnd) else {
        return;
    };
    let Some(last_line) = lines.len().checked_sub(1) else {
        return;
    };
    if let Some(state) = state_from_hwnd(hwnd) {
        state.text_selection = Some(TextSelection {
            anchor: TextPosition {
                line: 0,
                char_index: 0,
            },
            active: TextPosition {
                line: last_line,
                char_index: lines[last_line].text.chars().count(),
            },
        });
        InvalidateRect(hwnd, null(), 0);
    }
}

unsafe fn selected_text(hwnd: HWND) -> Option<String> {
    let selection = state_from_hwnd(hwnd)?.text_selection?;
    let (lines, _scroll_x) = selectable_lines_for_hwnd(hwnd)?;
    selected_text_from_lines(&lines, selection)
}

unsafe fn selectable_lines_for_hwnd(hwnd: HWND) -> Option<(Vec<SelectableLine>, i32)> {
    let layout = layout_for_hwnd(hwnd);
    let (text, scroll_x) = state_from_hwnd(hwnd)
        .map(|state| (CodeTextSnapshot::from_state(state), state.horizontal_scroll))?;
    Some((selectable_lines(text.as_code_text(), layout), scroll_x))
}

fn selected_text_from_lines(lines: &[SelectableLine], selection: TextSelection) -> Option<String> {
    let (start, end) = normalized_selection(selection, lines)?;
    if start == end {
        return None;
    }

    let mut output = String::new();
    for (line_index, line) in lines.iter().enumerate() {
        if line_index < start.line || line_index > end.line {
            continue;
        }

        if !output.is_empty() {
            output.push_str("\r\n");
        }

        let line_chars = line.text.chars().count();
        let start_char = if line_index == start.line {
            start.char_index.min(line_chars)
        } else {
            0
        };
        let end_char = if line_index == end.line {
            end.char_index.min(line_chars)
        } else {
            line_chars
        };
        output.push_str(&slice_chars(&line.text, start_char, end_char));
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn slice_chars(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

unsafe fn copy_to_clipboard(hwnd: HWND, text: &str) {
    let mut text = text.encode_utf16().collect::<Vec<_>>();
    text.push(0);
    let byte_len = text.len() * std::mem::size_of::<u16>();
    let handle = GlobalAlloc(GMEM_MOVEABLE, byte_len);
    if handle.is_null() {
        return;
    }

    let memory = GlobalLock(handle) as *mut u16;
    if memory.is_null() {
        GlobalFree(handle);
        return;
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), memory, text.len());
    GlobalUnlock(handle);

    if OpenClipboard(hwnd) == 0 {
        GlobalFree(handle);
        return;
    }
    EmptyClipboard();
    if SetClipboardData(CF_UNICODETEXT_FORMAT, handle).is_null() {
        GlobalFree(handle);
    }
    CloseClipboard();
}

fn point_in_rect(point: POINT, rect: RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn rect_has_area(rect: &RECT) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
}

fn point_from_lparam(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam & 0xffff) as u16 as i16 as i32,
        y: ((lparam >> 16) & 0xffff) as u16 as i16 as i32,
    }
}

fn update_clickable(state: &GuiState) -> bool {
    state.update.is_some() && !state.update_busy
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
                if id == ID_SRC_LABEL || id == ID_OUT_LABEL || id == ID_LANGUAGE_LABEL {
                    SetTextColor(hdc, COLOR_KEYWORD);
                    SetBkColor(hdc, COLOR_EDIT);
                    theme.edit as LRESULT
                } else {
                    SetTextColor(hdc, COLOR_MUTED);
                    SetBkColor(hdc, COLOR_EDIT);
                    theme.edit as LRESULT
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

struct ButtonSurface {
    brush: HBRUSH,
    background: ButtonBackground,
}

#[derive(Clone, Copy)]
enum ButtonBackground {
    Title,
}

unsafe fn draw_game_button_surface(
    hdc: HDC,
    bounds: &RECT,
    theme: &Theme,
    surface: ButtonSurface,
) -> RECT {
    let background_brush = match surface.background {
        ButtonBackground::Title => theme.title,
    };
    let shadow_soft = match surface.background {
        ButtonBackground::Title => theme.title_shadow_soft,
    };
    let shadow_deep = match surface.background {
        ButtonBackground::Title => theme.title_shadow_deep,
    };

    FillRect(hdc, bounds, background_brush);

    let shadow = 4;
    let soft = 2;
    let deep = 3;
    let face = RECT {
        left: bounds.left,
        top: bounds.top,
        right: bounds.right - shadow,
        bottom: bounds.bottom - shadow,
    };
    let bottom_shadow_soft = RECT {
        left: bounds.left + soft,
        top: face.bottom,
        right: bounds.right - deep,
        bottom: face.bottom + soft,
    };
    let bottom_shadow_deep = RECT {
        left: bounds.left + deep,
        top: face.bottom + soft,
        right: bounds.right - deep,
        bottom: bounds.bottom,
    };
    let right_shadow_soft = RECT {
        left: face.right,
        top: bounds.top + soft,
        right: face.right + soft,
        bottom: bounds.bottom - deep,
    };
    let right_shadow_deep = RECT {
        left: face.right + soft,
        top: bounds.top + deep,
        right: bounds.right,
        bottom: bounds.bottom - deep,
    };

    FillRect(hdc, &bottom_shadow_soft, shadow_soft);
    FillRect(hdc, &bottom_shadow_deep, shadow_deep);
    FillRect(hdc, &right_shadow_soft, shadow_soft);
    FillRect(hdc, &right_shadow_deep, shadow_deep);

    SelectObject(hdc, theme.no_outline as _);
    SelectObject(hdc, surface.brush as _);
    let radius = 5;
    RoundRect(
        hdc,
        face.left,
        face.top,
        face.right,
        face.bottom,
        radius,
        radius,
    );

    face
}

unsafe fn draw_close_icon(hdc: HDC, bounds: &RECT, background_color: u32) {
    const SAMPLES: i32 = 4;
    const ICON_SIZE: i32 = 19;
    const INSET: f32 = 3.5;
    const STROKE_RADIUS: f32 = 2.15;

    let left = bounds.left + ((bounds.right - bounds.left) - ICON_SIZE) / 2;
    let top = bounds.top + ((bounds.bottom - bounds.top) - ICON_SIZE) / 2;
    let max = ICON_SIZE as f32 - INSET;
    let sample_count = (SAMPLES * SAMPLES) as f32;

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let mut coverage = 0;
            for sample_y in 0..SAMPLES {
                for sample_x in 0..SAMPLES {
                    let px = x as f32 + (sample_x as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sample_y as f32 + 0.5) / SAMPLES as f32;
                    let down = distance_to_segment(px, py, INSET, INSET, max, max);
                    let up = distance_to_segment(px, py, max, INSET, INSET, max);
                    if down.min(up) <= STROKE_RADIUS {
                        coverage += 1;
                    }
                }
            }

            if coverage > 0 {
                let alpha = coverage as f32 / sample_count;
                let color = blend_color(COLOR_TEXT, background_color, alpha);
                let _ = SetPixel(hdc, left + x, top + y, color);
            }
        }
    }
}

fn distance_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = px - ax;
    let wy = py - ay;
    let length_squared = vx * vx + vy * vy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        ((wx * vx + wy * vy) / length_squared).clamp(0.0, 1.0)
    };
    let dx = px - (ax + t * vx);
    let dy = py - (ay + t * vy);
    (dx * dx + dy * dy).sqrt()
}

fn blend_color(foreground: u32, background: u32, alpha: f32) -> u32 {
    let blend_channel = |shift: u32| {
        let foreground = ((foreground >> shift) & 0xff) as f32;
        let background = ((background >> shift) & 0xff) as f32;
        (background + (foreground - background) * alpha).round() as u8
    };

    rgb(blend_channel(0), blend_channel(8), blend_channel(16))
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
    let initial_folder = state_from_hwnd(hwnd)
        .map(|state| {
            if is_src {
                state.config.src_dir.clone()
            } else {
                state.config.out_dir.clone()
            }
        })
        .filter(|path| Path::new(path).is_dir());

    let Some(path) = choose_folder(hwnd, initial_folder.as_deref()) else {
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

    save_config_only(hwnd);
}

unsafe fn cycle_language_mode(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    state.config.language = state.config.language.next();
    InvalidateRect(hwnd, null(), 0);
    save_config_only(hwnd);
}

unsafe fn choose_folder(hwnd: HWND, initial_folder: Option<&str>) -> Option<String> {
    let dialog: IFileOpenDialog =
        CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;

    let options =
        dialog.GetOptions().ok()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
    dialog.SetOptions(options).ok()?;

    let title = wide("フォルダを選択してください");
    let _ = dialog.SetTitle(PCWSTR(title.as_ptr()));

    if let Some(initial_folder) = initial_folder
        && let Some(folder) = shell_item_from_path(initial_folder)
    {
        let _ = dialog.SetFolder(&folder);
    }

    let owner = if hwnd.is_null() {
        None
    } else {
        Some(WinHwnd(hwnd))
    };
    dialog.Show(owner).ok()?;

    let item = dialog.GetResult().ok()?;
    shell_item_path(&item)
}

unsafe fn shell_item_from_path(path: &str) -> Option<IShellItem> {
    if !Path::new(path).is_dir() {
        return None;
    }

    let path = wide(path);
    SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None::<&IBindCtx>).ok()
}

unsafe fn shell_item_path(item: &IShellItem) -> Option<String> {
    let path = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
    let result = path.to_string().ok();
    WinCoTaskMemFree(Some(path.0 as *const c_void));
    result
}

unsafe fn start_update_check(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    if state.update_check_started {
        return;
    }
    state.update_check_started = true;

    let tx = state.tx.clone();
    thread::spawn(move || match updater::check_for_update() {
        Ok(check) if check.update_available => {
            let _ = tx.send(GuiEvent::UpdateAvailable(check.latest));
        }
        Ok(check) => {
            let _ = tx.send(GuiEvent::UpdateUnavailable(check.latest));
        }
        Err(_) => send_status(&tx, STATUS_UP_TO_DATE.to_string()),
    });
}

unsafe fn handle_update_clicked(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    if state.update_busy {
        return;
    }

    let Some(release) = state.update.clone() else {
        return;
    };
    let notes = if release.notes.trim().is_empty() {
        "リリースノートはありません。".to_string()
    } else {
        release.notes.clone()
    };
    let message = format!(
        "Transplanter {} が利用できます。\n\n{}\n\n更新しますか？",
        release.tag, notes
    );
    let title = wide("Transplanter 更新");
    let message = wide(&message);
    let answer = MessageBoxW(
        hwnd,
        message.as_ptr(),
        title.as_ptr(),
        MB_YESNO | MB_ICONINFORMATION,
    );
    if answer != IDYES {
        return;
    }

    state.update_busy = true;
    match updater::launch_update_script(&release) {
        Ok(()) => {
            DestroyWindow(hwnd);
        }
        Err(err) => {
            state.update_busy = false;
            show_update_error(hwnd, &err);
        }
    }
}

unsafe fn show_update_error(hwnd: HWND, message: &str) {
    let title = wide("Transplanter 更新");
    let message = wide(message);
    MessageBoxW(hwnd, message.as_ptr(), title.as_ptr(), MB_ICONERROR);
}

unsafe fn toggle_run(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };

    if state.active {
        deactivate_run(hwnd, state);
        return;
    }

    save_config_and_start(hwnd);
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
        save_config_only(hwnd);
    }
}

unsafe fn save_config_only(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    let Some(config) = persist_current_config(hwnd, state) else {
        return;
    };

    if state.active {
        deactivate_run(hwnd, state);
    }

    if let Err(err) = prepare_existing_workspace(&config) {
        set_diagnostic(hwnd, &err);
    }
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn save_config_and_start(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    let Some(config) = persist_current_config(hwnd, state) else {
        return;
    };

    if config.src_dir.is_empty() {
        deactivate_run(hwnd, state);
        set_diagnostic(hwnd, "エラー: ソースフォルダが空です");
        return;
    }

    let src_dir = PathBuf::from(&config.src_dir);

    if !src_dir.is_dir() {
        deactivate_run(hwnd, state);
        set_diagnostic(hwnd, "エラー: ソースフォルダが見つかりません");
        return;
    }

    if let Err(err) = prepare_language_workspace(&config) {
        deactivate_run(hwnd, state);
        set_diagnostic(hwnd, &err);
        return;
    }

    if config.out_dir.is_empty() {
        deactivate_run(hwnd, state);
        set_diagnostic(hwnd, "エラー: Saveフォルダを選択してください");
        return;
    }

    let out_dir = PathBuf::from(&config.out_dir);

    if let Err(err) = fs::create_dir_all(&out_dir) {
        deactivate_run(hwnd, state);
        set_diagnostic(
            hwnd,
            &format!("エラー: Save フォルダを作成できません: {err}"),
        );
        return;
    }

    if state
        .watcher
        .as_ref()
        .is_some_and(|watcher| watcher.matches(&src_dir, &out_dir, config.language))
    {
        state.active = true;
        clear_diagnostic(hwnd);
        invalidate_run_controls(hwnd);
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    stop_watcher(state);
    let stop = Arc::new(AtomicBool::new(false));
    let tx = state.tx.clone();
    let thread_stop = Arc::clone(&stop);
    let thread_src = src_dir.clone();
    let thread_out = out_dir.clone();
    let thread_language = config.language;
    let thread =
        thread::spawn(move || watch_loop(thread_src, thread_out, thread_language, thread_stop, tx));
    state.watcher = Some(WatchHandle {
        src_dir,
        out_dir,
        language: config.language,
        stop,
        thread: Some(thread),
    });
    state.active = true;
    clear_diagnostic(hwnd);
    invalidate_run_controls(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn sync_once(hwnd: HWND) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };

    let config = config_from_state(state);

    if config.src_dir.is_empty() || config.out_dir.is_empty() {
        set_diagnostic(
            hwnd,
            "エラー: ソースフォルダとSaveフォルダを選択してください",
        );
        return;
    }

    let src_dir = PathBuf::from(&config.src_dir);
    let out_dir = PathBuf::from(&config.out_dir);
    let result = (|| {
        prepare_language_workspace(&config)?;
        fs::create_dir_all(&out_dir)
            .map_err(|err| format!("エラー: Save フォルダを作成できません: {err}"))?;
        sync_project(&src_dir, &out_dir, config.language)
    })();

    match result {
        Ok(_count) => clear_diagnostic(hwnd),
        Err(err) => set_diagnostic(hwnd, &err),
    }
}

fn watch_loop(
    src_dir: PathBuf,
    out_dir: PathBuf,
    language: LanguageMode,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<GuiEvent>,
) {
    match sync_project(&src_dir, &out_dir, language) {
        Ok(count) => send_status(&tx, format!("OK: {count} 件を変換しました")),
        Err(err) => send_error(&tx, err),
    }

    let mut seen_sources = snapshot_sources_or_report(&src_dir, language, &tx);
    let mut seen_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &seen_sources, &tx);

    while !stop.load(Ordering::Relaxed) {
        sleep_until_next_poll(&stop);
        if stop.load(Ordering::Relaxed) {
            break;
        }

        let current_sources = snapshot_sources_or_report(&src_dir, language, &tx);
        let current_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &current_sources, &tx);

        if current_sources.keys().ne(seen_sources.keys())
            && language.includes_rust()
            && let Err(err) = write_manifest(&src_dir)
        {
            send_error(&tx, err);
        }

        let source_changed = current_sources
            .iter()
            .any(|(input_path, stamp)| seen_sources.get(input_path) != Some(stamp));
        if current_sources.keys().ne(seen_sources.keys()) || source_changed {
            match sync_project(&src_dir, &out_dir, language) {
                Ok(count) => send_status(&tx, format!("OK: {count} 件を更新しました")),
                Err(err) => send_error(&tx, err),
            }
            seen_sources = current_sources;
            seen_outputs = snapshot_outputs_or_report(&src_dir, &out_dir, &seen_sources, &tx);
            continue;
        }

        for input_path in current_sources.keys() {
            let Ok(output_path) = output_path_for(&src_dir, &out_dir, input_path) else {
                continue;
            };
            let output_changed =
                seen_outputs.get(&output_path) != current_outputs.get(&output_path);

            if output_changed {
                match compile_project_file(&src_dir, &out_dir, input_path, language) {
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
    language: LanguageMode,
    tx: &mpsc::Sender<GuiEvent>,
) -> std::collections::BTreeMap<PathBuf, FileStamp> {
    match snapshot_source_files(src_dir, language) {
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

    for marker in [".rs:", ".scm:", ".lisp:", ".py:"] {
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
    update_hover_from_cursor(hwnd);

    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };

    state.spinner = (state.spinner + 1) % 4;
    if state.active || needs_text_animation(state) {
        InvalidateRect(hwnd, null(), 0);
    }
}

fn needs_text_animation(state: &GuiState) -> bool {
    state.hover_target.is_some()
        || state.status_text.trim().is_empty()
        || state.config.src_dir.trim().is_empty()
        || state.config.out_dir.trim().is_empty()
        || update_clickable(state)
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
            GuiEvent::Status(message) => {
                clear_diagnostic(hwnd);
                set_status(hwnd, &message);
            }
            GuiEvent::Error(message) => set_diagnostic(hwnd, &message),
            GuiEvent::UpdateAvailable(release) => handle_update_available(hwnd, release),
            GuiEvent::UpdateUnavailable(release) => handle_update_unavailable(hwnd, release),
        }
    }
}

unsafe fn handle_update_available(hwnd: HWND, release: ReleaseInfo) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    state.config.last_release_tag = release.tag.clone();
    state.config.last_release_notes = release.notes.clone();
    state.update = Some(release.clone());
    let _ = write_config(&state.config_path, &state.config);
    set_status(hwnd, STATUS_UPDATE_AVAILABLE);
}

unsafe fn handle_update_unavailable(hwnd: HWND, release: ReleaseInfo) {
    let Some(state) = state_from_hwnd(hwnd) else {
        return;
    };
    state.config.last_release_tag = release.tag;
    state.config.last_release_notes = release.notes;
    state.update = None;
    let _ = write_config(&state.config_path, &state.config);
    set_status(hwnd, STATUS_UP_TO_DATE);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn stop_watcher(state: &mut GuiState) {
    if let Some(watcher) = state.watcher.take() {
        watcher.stop();
    }
}

unsafe fn deactivate_run(hwnd: HWND, state: &mut GuiState) {
    stop_watcher(state);
    state.active = false;
    invalidate_run_controls(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn persist_current_config(hwnd: HWND, state: &mut GuiState) -> Option<Config> {
    let config = config_from_state(state);
    state.config = config.clone();

    if let Err(err) = write_config(&state.config_path, &config) {
        deactivate_run(hwnd, state);
        set_diagnostic(hwnd, &err);
        return None;
    }

    Some(config)
}

fn config_from_state(state: &GuiState) -> Config {
    Config {
        src_dir: state.config.src_dir.trim().to_string(),
        out_dir: state.config.out_dir.trim().to_string(),
        language: state.config.language,
        last_release_tag: state.config.last_release_tag.clone(),
        last_release_notes: state.config.last_release_notes.clone(),
    }
}

unsafe fn state_from_hwnd(hwnd: HWND) -> Option<&'static mut GuiState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiState;
    ptr.as_mut()
}

unsafe fn set_status(hwnd: HWND, text: &str) {
    if !is_version_status(text) {
        InvalidateRect(hwnd, null(), 0);
        return;
    }

    if let Some(state) = state_from_hwnd(hwnd) {
        state.status_text = text.to_string();
    }
    invalidate_run_controls(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn set_diagnostic(hwnd: HWND, text: &str) {
    if let Some(state) = state_from_hwnd(hwnd) {
        state.diagnostic_text = text.to_string();
    }
    invalidate_run_controls(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn clear_diagnostic(hwnd: HWND) {
    if let Some(state) = state_from_hwnd(hwnd)
        && !state.diagnostic_text.is_empty()
    {
        state.diagnostic_text.clear();
        InvalidateRect(hwnd, null(), 0);
    }
}

fn is_version_status(text: &str) -> bool {
    text == STATUS_UPDATE_AVAILABLE || text == STATUS_UP_TO_DATE
}

unsafe fn invalidate_run_controls(parent: HWND) {
    InvalidateRect(parent, null(), 0);
}

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

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn with_theme<R>(f: impl FnOnce(&Theme) -> R) -> R {
    THEME.with(|theme| f(theme))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::SystemTime;

    #[test]
    fn lisp_diagnostics_are_compacted_for_gui() {
        let compact = compact_error_message(
            r"エラー: C:\Users\Player\farming\play_src\main.scm:2行1列: expected expression",
        );
        assert_eq!(compact, "エラー: main.scm:2行1列: expected expression");

        let display =
            diagnostic_display_value("エラー: main.scm:1行1列: bad\nGuile を確認してください")
                .unwrap();
        assert_eq!(
            display,
            "エラー: main.scm:1行1列: bad Guile を確認してください"
        );
    }

    #[test]
    fn selected_text_preserves_visible_line_order() {
        let lines = vec![
            test_selectable_line("status = \"最新のバージョンです\""),
            test_selectable_line("src_dir  = C:\\farm\\play_src"),
            test_selectable_line("out_dir  = C:\\game\\save"),
        ];
        let selected = selected_text_from_lines(
            &lines,
            TextSelection {
                anchor: TextPosition {
                    line: 0,
                    char_index: 10,
                },
                active: TextPosition {
                    line: 2,
                    char_index: 10,
                },
            },
        )
        .unwrap();

        assert_eq!(
            selected,
            "最新のバージョンです\"\r\nsrc_dir  = C:\\farm\\play_src\r\nout_dir  ="
        );
    }

    #[test]
    fn status_shows_spinner_while_checking_version() {
        assert_eq!(
            status_display_text("", false, 0).unwrap(),
            "status = \"確認中 /\""
        );
        assert_eq!(
            status_display_text("", false, 1).unwrap(),
            "status = \"確認中 -\""
        );
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
        let thread = thread::spawn(move || {
            watch_loop(src_dir, out_dir, LanguageMode::Rust, thread_stop, tx)
        });

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
        let thread = thread::spawn(move || {
            watch_loop(src_dir, out_dir, LanguageMode::Rust, thread_stop, tx)
        });

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
        let thread = thread::spawn(move || {
            watch_loop(src_dir, out_dir, LanguageMode::Rust, thread_stop, tx)
        });

        let message = recv_event_text(&rx);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        let _ = fs::remove_dir_all(workspace);

        assert!(message.contains("作成できません"), "{message}");
    }

    fn recv_event_text(rx: &mpsc::Receiver<GuiEvent>) -> String {
        loop {
            match rx.recv_timeout(Duration::from_secs(3)).unwrap() {
                GuiEvent::Status(message) | GuiEvent::Error(message) => return message,
                GuiEvent::UpdateAvailable(_) | GuiEvent::UpdateUnavailable(_) => {}
            }
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

    fn test_selectable_line(text: &str) -> SelectableLine {
        SelectableLine {
            text: text.to_string(),
            left: 0,
            top: 0,
            right: 800,
            bottom: 32,
            scrollable: false,
            font: SelectableFont::Code,
        }
    }
}
