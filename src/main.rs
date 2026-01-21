#![windows_subsystem = "windows"]
//! AMM - Anti-idle Mouse Mover

use rand::Rng;
use serde::Deserialize;
use std::{fs, mem, ptr, sync::atomic::{AtomicU8, Ordering}, thread, time::{Duration, Instant}};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    System::SystemInformation::GetTickCount,
    UI::Input::KeyboardAndMouse::*,
    UI::Shell::*,
    UI::WindowsAndMessaging::*,
};

const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY: u32 = 1;
const ID_TOGGLE: u32 = 100;
const ID_QUIT: u32 = 101;

#[derive(Deserialize, Clone)]
#[serde(default)]
struct Config {
    idle_threshold_ms: u64,
    interval_ms: u64,
    jitter_ms: u64,
    move_pattern: String,
    pause_on_fullscreen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { idle_threshold_ms: 120_000, interval_ms: 30_000, jitter_ms: 5_000, move_pattern: "ping_pong".into(), pause_on_fullscreen: true }
    }
}

static STATE: AtomicU8 = AtomicU8::new(0); // 0=运行, 1=暂停, 2=退出

fn get_idle_ms() -> u64 {
    let mut lii = LASTINPUTINFO { cbSize: mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe { if GetLastInputInfo(&mut lii) != 0 { (GetTickCount() - lii.dwTime) as u64 } else { 0 } }
}

fn get_cursor() -> POINT { let mut pt = POINT { x: 0, y: 0 }; unsafe { GetCursorPos(&mut pt) }; pt }

fn mouse_move(dx: i32, dy: i32) {
    let input = INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: 0, dwFlags: MOUSEEVENTF_MOVE, time: 0, dwExtraInfo: 0 } } };
    unsafe { SendInput(1, &input, mem::size_of::<INPUT>() as i32); }
}

fn is_fullscreen() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() { return false; }
        let mut r: RECT = mem::zeroed();
        GetWindowRect(fg, &mut r);
        r.left == 0 && r.top == 0 && r.right >= GetSystemMetrics(SM_CXSCREEN) && r.bottom >= GetSystemMetrics(SM_CYSCREEN)
    }
}

fn do_move(pattern: &str, rng: &mut impl Rng) {
    let origin = get_cursor();
    match pattern {
        "micro_jitter" => { let (dx, dy) = (rng.gen_range(-2..=2), rng.gen_range(-2..=2)); mouse_move(dx, dy); thread::sleep(Duration::from_millis(30)); mouse_move(-dx, -dy); }
        "random_walk_box" => { for _ in 0..4 { mouse_move(rng.gen_range(-3..=3), rng.gen_range(-3..=3)); thread::sleep(Duration::from_millis(20)); } let now = get_cursor(); mouse_move(origin.x - now.x, origin.y - now.y); }
        _ => { mouse_move(1, 0); thread::sleep(Duration::from_millis(50)); mouse_move(-1, 0); }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            if lparam as u32 == WM_RBUTTONUP {
                let mut pt: POINT = mem::zeroed();
                GetCursorPos(&mut pt);
                
                let menu = CreatePopupMenu();
                let state = STATE.load(Ordering::Relaxed);
                let toggle_text = if state == 0 { to_wide("暂停") } else { to_wide("继续") };
                AppendMenuW(menu, MF_STRING, ID_TOGGLE as usize, toggle_text.as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(menu, MF_STRING, ID_QUIT as usize, to_wide("退出").as_ptr());
                
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, ptr::null());
                DestroyMenu(menu);
            }
            0
        }
        WM_COMMAND => {
            match (wparam & 0xFFFF) as u32 {
                ID_TOGGLE => {
                    let s = STATE.load(Ordering::Relaxed);
                    STATE.store(if s == 0 { 1 } else { 0 }, Ordering::Relaxed);
                }
                ID_QUIT => {
                    STATE.store(2, Ordering::Relaxed);
                    PostQuitMessage(0);
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn main() {
    let config: Config = fs::read_to_string("amm.toml").ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
    
    unsafe {
        let class_name = to_wide("AMM_CLASS");
        let wc = WNDCLASSW {
            style: 0, lpfnWndProc: Some(wnd_proc), cbClsExtra: 0, cbWndExtra: 0,
            hInstance: GetModuleHandleW(ptr::null()), hIcon: 0 as _, hCursor: 0 as _,
            hbrBackground: 0 as _, lpszMenuName: ptr::null(), lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);
        
        let hwnd = CreateWindowExW(0, class_name.as_ptr(), to_wide("AMM").as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, 0 as _, GetModuleHandleW(ptr::null()), ptr::null());
        
        // 加载图标
        let icon: isize = LoadImageW(0 as _, to_wide("icon.ico").as_ptr(), IMAGE_ICON, 0, 0, LR_LOADFROMFILE | LR_DEFAULTSIZE) as isize;
        
        // 托盘图标
        let mut nid: NOTIFYICONDATAW = mem::zeroed();
        nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = ID_TRAY;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = if icon == 0 { LoadIconW(0 as _, IDI_APPLICATION) } else { icon as _ };
        let tip = to_wide("AMM - 运行中");
        for (i, &c) in tip.iter().take(127).enumerate() {
            nid.szTip[i] = c;
        }
        Shell_NotifyIconW(NIM_ADD, &nid);
        
        // 工作线程
        let cfg = config.clone();
        thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let mut last = Instant::now();
            loop {
                match STATE.load(Ordering::Relaxed) {
                    2 => break,
                    1 => { thread::sleep(Duration::from_millis(500)); continue; }
                    _ => {}
                }
                let idle = get_idle_ms();
                let interval = cfg.interval_ms + rng.gen_range(0..=cfg.jitter_ms);
                if idle >= cfg.idle_threshold_ms && last.elapsed() >= Duration::from_millis(interval) && !(cfg.pause_on_fullscreen && is_fullscreen()) {
                    do_move(&cfg.move_pattern, &mut rng);
                    last = Instant::now();
                }
                thread::sleep(Duration::from_millis(500));
            }
        });
        
        // 消息循环
        let mut msg: MSG = mem::zeroed();
        while GetMessageW(&mut msg, 0 as _, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // 清理
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}
