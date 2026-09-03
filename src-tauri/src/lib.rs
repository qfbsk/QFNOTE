#![allow(non_snake_case)]
#![allow(unused_variables)]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[cfg(windows)]
mod native_drag;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipItem {
    text: String,
    time: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuickFile {
    name: String,
    path: String,
    icon: Option<String>,
    id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuickLink {
    name: String,
    url: String,
    id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuickData {
    files: Vec<QuickFile>,
    links: Vec<QuickLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteFile {
    filePath: String,
    fileName: String,
    content: String,
}

pub struct AppState {
    save_dir: Mutex<String>,
    clip_history: Mutex<Vec<ClipItem>>,
    task_list: Mutex<Vec<serde_json::Value>>,
    quick_list: Mutex<QuickData>,
    current_theme: Mutex<i32>,
    theme_ready: Mutex<bool>,
    main_window_pos: Mutex<(i32, i32)>,
    current_tab: Mutex<i32>,
    initial_file: Mutex<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            save_dir: Mutex::new(String::new()),
            clip_history: Mutex::new(Vec::new()),
            task_list: Mutex::new(Vec::new()),
            quick_list: Mutex::new(QuickData::default()),
            current_theme: Mutex::new(0),
            theme_ready: Mutex::new(false),
            main_window_pos: Mutex::new((50, 50)),
            current_tab: Mutex::new(0),
            initial_file: Mutex::new(None),
        }
    }
}

fn get_save_dir() -> String {
    let docs = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
    let save_dir = docs.join("qfnote");
    if !save_dir.exists() {
        fs::create_dir_all(&save_dir).ok();
    }
    save_dir.to_string_lossy().to_string()
}

fn get_data_dir() -> String {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("qfnote");
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).ok();
    }
    data_dir.to_string_lossy().to_string()
}

#[cfg(target_os = "windows")]
fn setup_main_window(window: &tauri::WebviewWindow, app_handle: tauri::AppHandle) {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let hwnd = HWND(hwnd.0 as _);

            
            static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();
            
            unsafe extern "system" fn subclass_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_NCLBUTTONDBLCLK | WM_NCXBUTTONDBLCLK => {
                        return LRESULT(0);
                    }
                    WM_NCRBUTTONUP | WM_NCRBUTTONDOWN | WM_NCRBUTTONDBLCLK => {
                        return LRESULT(0);
                    }
                    WM_CONTEXTMENU => {
                        let x = (lparam.0 & 0xFFFF) as i32;
                        let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
                        let hit = SendMessageW(hwnd, WM_NCHITTEST, Some(WPARAM(0)), Some(LPARAM(((y as u32) << 16 | (x as u32)).try_into().unwrap())));
                        if hit.0 != HTCLIENT as isize {
                            return LRESULT(0);
                        }
                    }
                    _ => {}
                }
                if let Some(&orig_ptr) = ORIGINAL_WNDPROC.get() {
                    if orig_ptr != 0 {
                        let orig: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                            std::mem::transmute(orig_ptr as *const ());
                        return orig(hwnd, msg, wparam, lparam);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            let proc = subclass_proc as *const () as usize as isize;
            let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc);
            let _ = ORIGINAL_WNDPROC.set(old);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn setup_main_window(_window: &tauri::WebviewWindow, _app_handle: tauri::AppHandle) {}

#[cfg(target_os = "windows")]
fn disable_double_click_for_window(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let hwnd = HWND(hwnd.0 as _);
            
            static ORIGINAL_BALL_WNDPROC: OnceLock<isize> = OnceLock::new();
            
            unsafe extern "system" fn ball_subclass_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_NCLBUTTONDBLCLK | WM_NCXBUTTONDBLCLK | WM_LBUTTONDBLCLK => {
                        return LRESULT(0);
                    }
                    WM_NCRBUTTONUP | WM_NCRBUTTONDOWN | WM_NCRBUTTONDBLCLK | 
                    WM_RBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                        return LRESULT(0);
                    }
                    WM_CONTEXTMENU => {
                        return LRESULT(0);
                    }
                    _ => {}
                }
                if let Some(&orig_ptr) = ORIGINAL_BALL_WNDPROC.get() {
                    if orig_ptr != 0 {
                        let orig: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                            std::mem::transmute(orig_ptr as *const ());
                        return orig(hwnd, msg, wparam, lparam);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            let proc = ball_subclass_proc as *const () as usize as isize;
            let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc);
            let _ = ORIGINAL_BALL_WNDPROC.set(old);
            
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            SetWindowLongW(hwnd, GWL_STYLE, (style & !WS_MAXIMIZEBOX.0) as i32);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn disable_double_click_for_window(_window: &tauri::WebviewWindow) {}

#[tauri::command]
fn get_doc_path(state: State<AppState>) -> String {
    let mut save_dir = state.save_dir.lock().unwrap();
    if save_dir.is_empty() {
        let dir = get_save_dir();
        *save_dir = dir.clone();
        return dir;
    }
    save_dir.clone()
}

#[tauri::command]
fn save_note(content: String, filePath: String) -> Result<String, String> {
    fs::write(&filePath, content).map_err(|e| e.to_string())?;
    Ok(filePath)
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

fn get_image_ext_from_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/x-icon" => "ico",
        "image/tiff" => "tiff",
        _ => "png",
    }
}

fn get_image_ext_from_path(path: &str) -> String {
    let p = path.split('?').next().unwrap_or(path);
    let p = p.split('#').next().unwrap_or(p);
    std::path::Path::new(p)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| "png".to_string())
}

fn is_remote_url(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

fn is_base64_data(src: &str) -> bool {
    src.starts_with("data:image/")
}

pub(crate) fn decode_base64_image(src: &str) -> Result<(Vec<u8>, String), String> {
    let parts: Vec<&str> = src.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err("invalid base64 image".to_string());
    }
    let header = parts[0];
    let mime = header
        .strip_prefix("data:")
        .and_then(|h| h.split(';').next())
        .unwrap_or("image/png");
    let ext = get_image_ext_from_mime(mime).to_string();
    let data = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|e| e.to_string())?;
    Ok((data, ext))
}

fn extract_asset_path(src: &str) -> Option<String> {
    if let Some(rest) = src.strip_prefix("asset://") {
        let decoded = urlencoding::decode(rest).unwrap_or_else(|_| std::borrow::Cow::Borrowed(rest));
        let path = decoded.into_owned();
        let path = path.replace('/', "\\");
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
        let path = path.trim_start_matches('\\').to_string();
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
        return None;
    }
    if let Some(rest) = src.strip_prefix("file:///") {
        let decoded = urlencoding::decode(rest).unwrap_or_else(|_| std::borrow::Cow::Borrowed(rest));
        let path = decoded.into_owned();
        let path = path.replace('/', "\\");
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
        return None;
    }
    if let Some(rest) = src.strip_prefix("file://") {
        let decoded = urlencoding::decode(rest).unwrap_or_else(|_| std::borrow::Cow::Borrowed(rest));
        let path = decoded.into_owned();
        let path = path.replace('/', "\\");
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
        return None;
    }
    None
}

fn download_image(url: &str) -> Result<(Vec<u8>, String), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    let ext = get_image_ext_from_mime(&mime).to_string();
    let bytes = resp.bytes().map_err(|e| e.to_string())?.to_vec();
    Ok((bytes, ext))
}

#[cfg(target_os = "windows")]
static KEEP_AWAKE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn toggle_keep_awake() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Power::SetThreadExecutionState;
        use windows::Win32::System::Power::{ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};

        unsafe {
            if KEEP_AWAKE_ACTIVE.load(Ordering::SeqCst) {
                SetThreadExecutionState(ES_CONTINUOUS);
                KEEP_AWAKE_ACTIVE.store(false, Ordering::SeqCst);
            } else {
                SetThreadExecutionState(
                    ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED | ES_AWAYMODE_REQUIRED,
                );
                KEEP_AWAKE_ACTIVE.store(true, Ordering::SeqCst);
            }
            Ok(KEEP_AWAKE_ACTIVE.load(Ordering::SeqCst))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Not supported on this platform".to_string())
    }
}

#[tauri::command]
fn export_note_with_images(content: String, out_dir: String, base_name: String) -> Result<String, String> {
    let out_path = std::path::Path::new(&out_dir);
    let img_dir = out_path.join("images");
    fs::create_dir_all(&img_dir).map_err(|e| e.to_string())?;

    let mut new_content = content.clone();
    let mut img_counter = 0;
    let mut used_names = std::collections::HashSet::new();

    let re = regex::Regex::new(r"!\[([^\]]*)\]\(([^)]+)\)").unwrap();
    let mut replacements: Vec<(String, String)> = Vec::new();

    for cap in re.captures_iter(&content) {
        let full_match = cap.get(0).unwrap().as_str().to_string();
        let alt = cap.get(1).unwrap().as_str().to_string();
        let src = cap.get(2).unwrap().as_str().to_string();

        if replacements.iter().any(|(o, _)| o == &full_match) {
            continue;
        }

        let result: Result<(Vec<u8>, String), String> = if is_base64_data(&src) {
            decode_base64_image(&src)
        } else if is_remote_url(&src) {
            download_image(&src)
        } else if let Some(local_path) = extract_asset_path(&src) {
            let ext = get_image_ext_from_path(&local_path);
            let data = fs::read(&local_path).map_err(|e| e.to_string())?;
            Ok((data, ext))
        } else {
            continue;
        };

        match result {
            Ok((data, ext)) => {
                img_counter += 1;
                let mut img_name = format!("img_{}.{}", img_counter, ext);
                let mut i = 1;
                while used_names.contains(&img_name) {
                    img_name = format!("img_{}_{}.{}", img_counter, i, ext);
                    i += 1;
                }
                used_names.insert(img_name.clone());

                let img_path = img_dir.join(&img_name);
                fs::write(&img_path, &data).map_err(|e| e.to_string())?;

                let new_src = format!("images/{}", img_name);
                let new_md = format!("![{}]({})", alt, new_src);
                replacements.push((full_match, new_md));
            }
            Err(_) => continue,
        }
    }

    for (old, new) in &replacements {
        new_content = new_content.replace(old, new);
    }

    let md_path = out_path.join(format!("{}.md", base_name));
    fs::write(&md_path, &new_content).map_err(|e| e.to_string())?;

    Ok(md_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn open_file_dialog(app: AppHandle) -> Result<NoteFile, String> {
    use tauri_plugin_dialog::DialogExt;

    let save_dir = get_save_dir();

    let file_path = app
        .dialog()
        .file()
        .add_filter("Markdown文件", &["md"])
        .add_filter("所有文件", &["*"])
        .set_directory(&save_dir)
        .blocking_pick_file();

    if let Some(path) = file_path {
        let path_str = path.to_string();
        let file_name = std::path::Path::new(&path_str)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名.md".to_string());

        let content = fs::read_to_string(&path_str).unwrap_or_default();

        return Ok(NoteFile {
            filePath: path_str,
            fileName: file_name,
            content,
        });
    }

    Err("No file selected".to_string())
}

#[tauri::command]
async fn select_folder_dialog(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .set_title("选择导出存放目录")
        .blocking_pick_folder();

    if let Some(path) = folder {
        return Ok(path.to_string());
    }

    Err("No folder selected".to_string())
}

#[tauri::command]
fn pick_files_dialog(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("所有文件", &["*"])
        .blocking_pick_files();

    if let Some(paths) = files {
        let result: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
        return Ok(result);
    }

    Err("No files selected".to_string())
}

#[tauri::command]
fn pick_folder_dialog(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .blocking_pick_folder();

    if let Some(path) = folder {
        return Ok(path.to_string());
    }

    Err("No folder selected".to_string())
}

#[tauri::command]
fn get_latest_file() -> Result<Option<NoteFile>, String> {
    let save_dir = get_save_dir();

    let entries = fs::read_dir(&save_dir).map_err(|e| e.to_string())?;
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();

    if files.is_empty() {
        return Ok(None);
    }

    files.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    let latest = &files[0];
    let path = latest.path();
    let path_str = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let content = fs::read_to_string(&path_str).unwrap_or_default();

    Ok(Some(NoteFile {
        filePath: path_str,
        fileName: file_name,
        content,
    }))
}

#[tauri::command]
fn read_clip_text(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_clipboard::Clipboard;
    let clipboard = app.state::<Clipboard>();
    match clipboard.read_text() {
        Ok(text) => Ok(text.trim().to_string()),
        Err(_) => Ok(String::new()),
    }
}

#[tauri::command]
fn write_clip_text(text: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard::Clipboard;
    let clipboard = app.state::<Clipboard>();
    clipboard.write_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn push_clip_item(item: ClipItem, app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut history = state.clip_history.lock().unwrap();
    if history.iter().any(|i| i.text == item.text) {
        return Ok(());
    }
    history.insert(0, item.clone());
    if history.len() > 20 {
        history.pop();
    }

    let data_dir = get_data_dir();
    let history_file = std::path::Path::new(&data_dir).join("clip_history.json");
    let json = serde_json::to_string_pretty(&*history).map_err(|e| e.to_string())?;
    fs::write(&history_file, json).ok();

    app.emit("clip-add", item).ok();
    Ok(())
}

#[tauri::command]
fn get_clip_history(state: State<AppState>) -> Result<Vec<ClipItem>, String> {
    let data_dir = get_data_dir();
    let history_file = std::path::Path::new(&data_dir).join("clip_history.json");

    if history_file.exists() {
        let content = fs::read_to_string(&history_file).map_err(|e| e.to_string())?;
        let items: Vec<ClipItem> = serde_json::from_str(&content).unwrap_or_default();
        let mut history = state.clip_history.lock().unwrap();
        *history = items.clone();
        return Ok(items);
    }

    Ok(vec![])
}

#[tauri::command]
fn get_task_list(state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let data_dir = get_data_dir();
    let task_file = std::path::Path::new(&data_dir).join("task_list.json");

    if task_file.exists() {
        let content = fs::read_to_string(&task_file).map_err(|e| e.to_string())?;
        let tasks: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap_or_default();
        let mut list = state.task_list.lock().unwrap();
        *list = tasks.clone();
        return Ok(tasks);
    }

    Ok(vec![])
}

#[tauri::command]
fn save_task_list(list: Vec<serde_json::Value>, state: State<AppState>) -> Result<(), String> {
    let data_dir = get_data_dir();
    let task_file = std::path::Path::new(&data_dir).join("task_list.json");
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    fs::write(&task_file, json).map_err(|e| e.to_string())?;
    let mut state_list = state.task_list.lock().unwrap();
    *state_list = list;
    Ok(())
}

#[tauri::command]
fn get_quick_list(state: State<AppState>) -> Result<QuickData, String> {
    let data_dir = get_data_dir();
    let quick_file = std::path::Path::new(&data_dir).join("quick_list.json");

    if quick_file.exists() {
        let content = fs::read_to_string(&quick_file).map_err(|e| e.to_string())?;
        let data: QuickData = serde_json::from_str(&content).unwrap_or_default();
        let mut quick = state.quick_list.lock().unwrap();
        *quick = data.clone();
        return Ok(data);
    }

    Ok(QuickData::default())
}

#[tauri::command]
fn save_quick_list(data: QuickData, state: State<AppState>) -> Result<(), String> {
    let data_dir = get_data_dir();
    let quick_file = std::path::Path::new(&data_dir).join("quick_list.json");
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&quick_file, json).map_err(|e| e.to_string())?;
    let mut quick = state.quick_list.lock().unwrap();
    *quick = data;
    Ok(())
}

#[tauri::command]
fn open_path(path: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_external(url: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_icon_to_base64(hicon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<String> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use image::{ImageBuffer, Rgba};

    unsafe {
        let mut icon_info = ICONINFO::default();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            return None;
        }

        let hbm_color = icon_info.hbmColor;
        let hbm_mask = icon_info.hbmMask;

        if hbm_color.is_invalid() {
            if !hbm_mask.is_invalid() {
                let _ = DeleteObject(hbm_mask.into());
            }
            return None;
        }

        let mut bmp = BITMAP::default();
        if GetObjectW(hbm_color.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bmp as *mut _ as *mut _)) == 0 {
            let _ = DeleteObject(hbm_color.into());
            if !hbm_mask.is_invalid() {
                let _ = DeleteObject(hbm_mask.into());
            }
            return None;
        }

        let width = bmp.bmWidth as u32;
        let height = bmp.bmHeight as u32;

        let hdc = GetDC(None);
        let hdc_mem = CreateCompatibleDC(Some(hdc));
        let _ = SelectObject(hdc_mem, hbm_color.into());

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = bmp.bmWidth;
        bmi.bmiHeader.biHeight = bmp.bmHeight;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let pixel_count = (width * height) as usize;
        let mut pixels: Vec<u32> = vec![0; pixel_count];

        let result = GetDIBits(
            hdc_mem,
            hbm_color,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(None, hdc);
        let _ = DeleteObject(hbm_color.into());
        if !hbm_mask.is_invalid() {
            let _ = DeleteObject(hbm_mask.into());
        }

        if result == 0 {
            return None;
        }

        let mut rgba_pixels: Vec<u8> = Vec::with_capacity(pixel_count * 4);
        for y in 0..height {
            for x in 0..width {
                let idx = ((height - 1 - y) * width + x) as usize;
                let pixel = pixels[idx];
                let b = (pixel & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let r = ((pixel >> 16) & 0xFF) as u8;
                let a = ((pixel >> 24) & 0xFF) as u8;
                rgba_pixels.extend_from_slice(&[r, g, b, a]);
            }
        }

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba_pixels)?;

        let mut png_data: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_data);
        if img.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
            return None;
        }

        let base64_str = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Some(format!("data:image/png;base64,{}", base64_str))
    }
}

#[cfg(target_os = "windows")]


struct ComGuard {
    need_uninit: bool,
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.need_uninit {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

fn resolve_lnk_target(lnk_path: &str) -> Option<String> {
    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::Storage::FileSystem::*;
    
    unsafe {
        
        
        
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            return None;
        }
        
        let _guard = ComGuard { need_uninit: true };

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_ALL).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        
        let wide_path: Vec<u16> = lnk_path.encode_utf16().chain(std::iter::once(0)).collect();
        persist.Load(PCWSTR::from_raw(wide_path.as_ptr()), STGM_READ).ok()?;
        
        let mut path_buf = [0u16; 520];
        let mut find_data: WIN32_FIND_DATAW = std::mem::zeroed();
        link.GetPath(&mut path_buf, &mut find_data, SLGP_UNCPRIORITY.0 as u32).ok()?;
        
        let path_str = String::from_utf16_lossy(&path_buf[..path_buf.iter().position(|&c| c == 0).unwrap_or(0)]);
        
        if path_str.is_empty() {
            None
        } else {
            Some(path_str)
        }
    }
}

#[tauri::command]
fn get_file_icon(path: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::*;
        use windows::Win32::UI::Shell::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::Win32::Storage::FileSystem::*;
        use std::path::Path;

        let is_lnk = path.to_lowercase().ends_with(".lnk");
        
        let mut icon_path = path.clone();
        let mut need_link_overlay = false;
        
        if is_lnk {
            if let Some(resolved) = resolve_lnk_target(&path) {
                if Path::new(&resolved).exists() {
                    icon_path = resolved;
                    need_link_overlay = true;
                }
            }
        }

        let file_path = Path::new(&icon_path);
        let file_exists = file_path.exists();

        let wide_path: Vec<u16> = icon_path.encode_utf16().chain(std::iter::once(0)).collect();
        
        let mut shfi = SHFILEINFOW::default();
        
        let mut flags = SHGFI_ICON | SHGFI_LARGEICON;
        if !file_exists {
            flags |= SHGFI_USEFILEATTRIBUTES;
        }
        if need_link_overlay {
            flags |= SHGFI_LINKOVERLAY;
        }
        
        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR::from_raw(wide_path.as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
                Some(&mut shfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                flags,
            )
        };

        if result == 0 || shfi.hIcon.is_invalid() {
            let orig_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let result2 = unsafe {
                SHGetFileInfoW(
                    PCWSTR::from_raw(orig_wide.as_ptr()),
                    FILE_ATTRIBUTE_NORMAL,
                    Some(&mut shfi),
                    std::mem::size_of::<SHFILEINFOW>() as u32,
                    SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
                )
            };
            if result2 == 0 || shfi.hIcon.is_invalid() {
                return Ok(None);
            }
        }

        let base64 = extract_icon_to_base64(shfi.hIcon);
        
        unsafe {
            let _ = DestroyIcon(shfi.hIcon);
        }

        Ok(base64)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(None)
    }
}

#[tauri::command]
fn resolve_lnk(path: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(resolve_lnk_target(&path))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(None)
    }
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        use std::env;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        let key = hkcu.open_subkey_with_flags(path, KEY_WRITE)
            .map_err(|e| e.to_string())?;

        if enabled {
            if let Ok(exe_path) = env::current_exe() {
                key.set_value("清枫速记", &exe_path.to_string_lossy().to_string())
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let _ = key.delete_value("清枫速记");
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[tauri::command]
fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        use std::process::Command;
        use std::fs::File;
        use std::io::Write;
        use std::os::windows::process::CommandExt;

        let file_path = Path::new(&path);
        if !file_path.exists() {
            return Err("文件不存在".into());
        }

        let temp_script = std::env::temp_dir().join("qfnote_copy.ps1");
        let script_content = format!(
            "Set-Clipboard -Path '{}'\r\n",
            path.replace("'", "''")
        );

        if let Ok(mut f) = File::create(&temp_script) {
            let _ = f.write_all(script_content.as_bytes());
        }

        let status = Command::new("powershell")
            .args(&["-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", temp_script.to_str().unwrap_or("")])
            .creation_flags(0x08000000)
            .status()
            .map_err(|e| format!("执行PowerShell失败: {}", e))?;

        let _ = std::fs::remove_file(&temp_script);

        if status.success() {
            Ok(())
        } else {
            Err("复制文件失败".into())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(())
    }
}





fn fetch_image_bytes_to_raw(url: &str) -> Result<(Vec<u8>, String), String> {
    use reqwest::blocking::Client;
    use reqwest::header as rh;
    let client = Client::builder().build().map_err(|e| e.to_string())?;
    let resp = client.get(url).header("User-Agent", "Mozilla/5.0").send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status())); }
    let mime = resp.headers().get(rh::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or("image/png").to_string())
        .unwrap_or_else(|| "image/png".to_string());
    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    Ok((bytes.to_vec(), mime))
}


#[tauri::command]
fn fetch_image_bytes(url: String) -> Result<String, String> {
    let (bytes, mime) = fetch_image_bytes_to_raw(&url)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}


#[tauri::command]
fn fetch_url_html(url: String) -> Result<String, String> {
    use reqwest::blocking::Client;
    use std::time::Duration;
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}



#[tauri::command]
fn fetch_images_as_dataurls(urls: Vec<String>, referer: Option<String>) -> std::collections::HashMap<String, String> {
    use reqwest::blocking::Client;
    use reqwest::header as rh;
    use std::time::Duration;
    let mut map = std::collections::HashMap::new();
    let client = match Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build() {
        Ok(c) => c,
        Err(_) => return map,
    };
    for url in urls {
        let mut req = client.get(&url);
        if let Some(r) = referer.as_ref() {
            if !r.is_empty() {
                req = req.header(rh::REFERER, r.clone());
            }
        }
        req = req.header(rh::ACCEPT, "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8");
        req = req.header(rh::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8");
        if let Ok(resp) = req.send() {
            if resp.status().is_success() {
                
                
                let mime = resp.headers().get(rh::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.split(';').next().unwrap_or("image/png").to_string())
                    .unwrap_or_else(|| "image/png".to_string());
                if let Ok(bytes) = resp.bytes() {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    map.insert(url.clone(), format!("data:{};base64,{}", mime, b64));
                }
            }
        }
    }
    map
}


#[cfg(windows)]
fn drag_log(msg: &str) {
    use std::io::Write;
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("qfnote_drag.log"))
        .and_then(|mut f| writeln!(f, "[qfnote] {}", msg));
}




#[cfg(windows)]
#[derive(serde::Deserialize)]
struct DragItem {
    src: String,
    data_url: Option<String>,
}



#[cfg(windows)]
fn random_name_16() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let n = nanos as u64;
    let mut x = (n ^ (seq.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ 0x1234_5678_9ABC_DEF0) as u64;
    if x == 0 {
        x = 0xABCDEF1234567890;
    }
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut s = String::with_capacity(16);
    for _ in 0..16 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(CHARS[(x % CHARS.len() as u64) as usize] as char);
    }
    s
}

#[cfg(windows)]
#[tauri::command]
fn start_native_file_drag(items: Vec<DragItem>, app: AppHandle) -> Result<(), String> {
    drag_log(&format!("start_native_file_drag called: item_count={}", items.len()));
    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(items.len());
    for item in items.iter() {
        
        let src_data = if item.data_url.as_deref().map(|s| s.starts_with("data:image")).unwrap_or(false) {
            item.data_url.as_deref()
        } else if item.src.starts_with("data:image") {
            Some(item.src.as_str())
        } else {
            None
        };
        let decoded = match src_data {
            Some(s) => match decode_base64_image(s) {
                Ok(d) => Some(d),
                Err(_) => None,
            },
            None => None,
        };
        let (bytes, ext): (Vec<u8>, String) = if let Some(d) = decoded {
            drag_log("bytes from cached data URL");
            d
        } else if item.src.starts_with("http://") || item.src.starts_with("https://") {
            match fetch_image_bytes_to_raw(&item.src) {
                Ok((b, mime)) => {
                    drag_log(&format!("bytes from network fetch: {} bytes, mime={}", b.len(), mime));
                    (b, get_image_ext_from_mime(&mime).to_string())
                }
                Err(e) => {
                    drag_log(&format!("NETWORK FETCH FAIL: {} -> skip", e));
                    continue;
                }
            }
        } else if let Some(p) = extract_asset_path(&item.src) {
            match std::fs::read(&p) {
                Ok(data) => {
                    drag_log(&format!("bytes from local/asset file {}: {} bytes", p, data.len()));
                    let ext = std::path::Path::new(&p)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png")
                        .to_string();
                    (data, ext)
                }
                Err(e) => {
                    drag_log(&format!("LOCAL READ FAIL: {} -> skip", e));
                    continue;
                }
            }
        } else {
            drag_log(&format!("NO BYTES: src={}", item.src));
            continue;
        };
        let rnd = random_name_16();
        let fname = format!("{}.{}", rnd, ext);
        drag_log(&format!("drag file ready: fname={} bytes={}", fname, bytes.len()));
        files.push((fname, bytes));
    }
    if files.is_empty() {
        drag_log("NO FILES: all items failed to get bytes");
        return Err("没有图片能取到字节，无法拖出".to_string());
    }
    drag_log(&format!("drag files total={}", files.len()));
    
    let _ = app.run_on_main_thread(move || {
        drag_log("run_on_main_thread -> calling do_file_drag");
        let _ = crate::native_drag::do_file_drag(files);
    });
    Ok(())
}

#[tauri::command]
fn set_win_title(title: String, app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_title(&title).ok();
    }
    Ok(())
}

#[tauri::command]
fn tab_change(tab_index: i32, state: State<AppState>) -> Result<(), String> {
    let mut tab = state.current_tab.lock().unwrap();
    *tab = tab_index;
    Ok(())
}


const THEME_MAX: i32 = 6;

#[tauri::command]
fn sync_theme(theme: i32, app: AppHandle, state: State<AppState>) -> Result<(), String> {
    {
        let mut current = state.current_theme.lock().unwrap();
        *current = theme;
    }
    let theme_path = PathBuf::from(get_data_dir()).join("theme.txt");
    fs::write(&theme_path, theme.to_string()).map_err(|e| e.to_string())?;
    
    let theme_next_path = PathBuf::from(get_data_dir()).join("theme_next.txt");
    let _ = fs::write(&theme_next_path, ((theme + 1) % THEME_MAX).to_string());
    if let Some(ball) = app.get_webview_window("ball") {
        ball.emit("sync-theme", theme).ok();
    }
    Ok(())
}

#[tauri::command]
fn to_ball(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let main_pos_x;
    let main_pos_y;
    let main_width;
    let main_height;
    
    if let Some(main_win) = app.get_webview_window("main") {
        if let Ok(pos) = main_win.outer_position() {
            let mut saved_pos = state.main_window_pos.lock().unwrap();
            *saved_pos = (pos.x, pos.y);
            main_pos_x = pos.x as f64;
            main_pos_y = pos.y as f64;
        } else {
            let pos = state.main_window_pos.lock().unwrap();
            main_pos_x = pos.0 as f64;
            main_pos_y = pos.1 as f64;
        }
        
        if let Ok(size) = main_win.outer_size() {
            main_width = size.width as f64;
            main_height = size.height as f64;
        } else {
            main_width = 280.0;
            main_height = 360.0;
        }
        
        main_win.hide().ok();
    } else {
        let pos = state.main_window_pos.lock().unwrap();
        main_pos_x = pos.0 as f64;
        main_pos_y = pos.1 as f64;
        main_width = 280.0;
        main_height = 360.0;
    }

    if let Some(ball_win) = app.get_webview_window("ball") {
        ball_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(48.0, 48.0))).ok();
        
        let ball_size = 48.0;
        
        let ball_x = main_pos_x + (main_width - ball_size) / 2.0;
        let ball_y = main_pos_y + (main_height - ball_size) / 2.0;
        
        let (screen_width, screen_height) = get_screen_size(&app);
        
        let final_x = if ball_x < 0.0 { 0.0 } else if ball_x + ball_size > screen_width { screen_width - ball_size } else { ball_x };
        let final_y = if ball_y < 0.0 { 0.0 } else if ball_y + ball_size > screen_height { screen_height - ball_size } else { ball_y };
        
        ball_win
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(final_x as i32, final_y as i32)))
            .ok();
        ball_win.show().ok();
        ball_win.set_focus().ok();
    }

    Ok(())
}

fn get_screen_size(app: &AppHandle) -> (f64, f64) {
    
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let mut rect = windows::Win32::Foundation::RECT::default();
            let _ = SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut rect as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
            return (rect.right as f64 - rect.left as f64, rect.bottom as f64 - rect.top as f64);
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        
        (1920.0, 1080.0)
    }
}

#[tauri::command]
fn to_main(app: AppHandle) -> Result<(), String> {
    if let Some(ball_win) = app.get_webview_window("ball") {
        ball_win.hide().ok();
    }

    if let Some(main_win) = app.get_webview_window("main") {
        main_win.show().ok();
        main_win.set_focus().ok();
    }

    Ok(())
}

#[tauri::command]
fn open_file_location(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let normalized_path = path.replace('/', "\\");
        Command::new("explorer")
            .arg(format!("/select,{}", normalized_path))
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(())
    }
}

#[tauri::command]
fn drag_start_minimize(app: AppHandle) -> Result<(), String> {
    app.emit("drag-out", ()).ok();
    Ok(())
}

#[tauri::command]
fn drag_end_restore(app: AppHandle) -> Result<(), String> {
    app.emit("drag-end", ()).ok();
    Ok(())
}

#[tauri::command]
fn mini_esc_close(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}


fn read_theme_next() -> i32 {
    let p = PathBuf::from(get_data_dir()).join("theme_next.txt");
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(n) = s.trim().parse::<i32>() {
            if n >= 0 && n < THEME_MAX {
                return n;
            }
        }
    }
    0
}



fn persist_theme(used: i32) {
    let _ = fs::write(
        PathBuf::from(get_data_dir()).join("theme.txt"),
        used.to_string(),
    );
    let _ = fs::write(
        PathBuf::from(get_data_dir()).join("theme_next.txt"),
        ((used + 1) % THEME_MAX).to_string(),
    );
}

fn resolve_init_theme(app: &AppHandle, state: &State<AppState>) -> i32 {
    let mut ready = state.theme_ready.lock().unwrap();
    if *ready {
        return *state.current_theme.lock().unwrap();
    }
    let args: Vec<String> = std::env::args().collect();
    let mut theme_from_arg: Option<i32> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--theme" && i + 1 < args.len() {
            if let Ok(t) = args[i + 1].parse::<i32>() {
                if t >= 0 && t < THEME_MAX {
                    theme_from_arg = Some(t);
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let init_theme = theme_from_arg.unwrap_or_else(read_theme_next);
    persist_theme(init_theme);
    *state.current_theme.lock().unwrap() = init_theme;
    *ready = true;
    let _ = app.emit("init-theme", init_theme);
    init_theme
}

#[tauri::command]
fn render_ready(app: AppHandle) -> Result<i32, String> {
    let save_dir = get_save_dir();
    let state: State<AppState> = app.state();
    *state.save_dir.lock().unwrap() = save_dir;

    let init_theme = resolve_init_theme(&app, &state);

    let args: Vec<String> = std::env::args().collect();
    for arg in &args {
        if arg.ends_with(".md") && std::path::Path::new(arg).exists() {
            *state.initial_file.lock().unwrap() = Some(arg.clone());
            let _ = app.emit("open-external-file", arg.clone());
            break;
        }
    }

    Ok(init_theme)
}

#[tauri::command]
fn get_current_theme(app: AppHandle, state: State<AppState>) -> i32 {
    resolve_init_theme(&app, &state)
}

#[tauri::command]
fn get_initial_file(state: State<AppState>) -> Option<String> {
    state.initial_file.lock().unwrap().clone()
}



const TUTORIAL_MD: &str = include_str!("../../清枫速记使用教程.md");

#[tauri::command]
fn get_tutorial_content() -> String {
    TUTORIAL_MD.to_string()
}



#[tauri::command]
fn is_first_launch() -> bool {
    let save_dir = get_save_dir();
    let marker = std::path::Path::new(&save_dir).join(".qfnote_first_launch");
    if marker.exists() {
        return false;
    }
    let _ = fs::create_dir_all(&save_dir);
    let _ = fs::write(&marker, "1");
    true
}

#[tauri::command]
fn launch_new_instance(
    file_path: Option<String>,
    _theme: Option<i32>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(&exe_path);
    if let Some(ref path) = file_path {
        cmd.arg(path);
    }
    
    
    
    let next = read_theme_next();
    cmd.arg("--theme").arg(next.to_string());
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn capture_screen() -> Result<Option<String>, String> {
    use screenshots::Screen;

    let screens = Screen::all().map_err(|e| e.to_string())?;
    if let Some(screen) = screens.first() {
        let image = screen.capture().map_err(|e| e.to_string())?;
        let width = image.width();
        let height = image.height();
        let raw_data = image.into_raw();

        let img = image::RgbaImage::from_raw(width, height, raw_data)
            .ok_or("Failed to create image")?;

        let mut buffer = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;

        let base64_str = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());
        return Ok(Some(format!("data:image/png;base64,{}", base64_str)));
    }

    Ok(None)
}

#[tauri::command]
fn mini_insert_screenshot(imgBase64: String, app: AppHandle) -> Result<(), String> {
    
    if let Some(main_win) = app.get_webview_window("main") {
        main_win.emit("insert-screenshot", imgBase64).ok();
    }
    Ok(())
}

fn keep_top(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        main.set_always_on_top(true).ok();
        main.set_visible_on_all_workspaces(true).ok();
        main.set_skip_taskbar(true).ok();
    }
    if let Some(ball) = app.get_webview_window("ball") {
        ball.set_always_on_top(true).ok();
        ball.set_visible_on_all_workspaces(true).ok();
        ball.set_skip_taskbar(true).ok();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_doc_path,
            save_note,
            read_file,
            write_file,
            export_note_with_images,
            toggle_keep_awake,
            open_file_dialog,
            select_folder_dialog,
            pick_files_dialog,
            pick_folder_dialog,
            get_latest_file,
            get_initial_file,
            get_tutorial_content,
            is_first_launch,
            read_clip_text,
            write_clip_text,
            push_clip_item,
            get_clip_history,
            get_task_list,
            save_task_list,
            get_quick_list,
            save_quick_list,
            open_path,
            open_external,
            get_file_icon,
            resolve_lnk,
            set_autostart,
            copy_file_to_clipboard,
            start_native_file_drag,
            fetch_image_bytes,
            fetch_url_html,
            fetch_images_as_dataurls,
            open_file_location,
            set_win_title,
            tab_change,
            sync_theme,
            to_ball,
            to_main,
            drag_start_minimize,
            drag_end_restore,
            mini_esc_close,
            render_ready,
            get_current_theme,
            capture_screen,
            launch_new_instance,
            mini_insert_screenshot,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = get_data_dir();

            if let Some(main_win) = app.get_webview_window("main") {
                setup_main_window(&main_win, handle.clone());

                let main_handle = handle.clone();
                main_win.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                            main_handle.emit("force-focus-editor", ()).ok();
                        }
                        _ => {}
                    }
                });
            }
            
            let ball_window = WebviewWindowBuilder::new(&handle, "ball", WebviewUrl::App("mini.html".into()))
                .title("悬浮圆球")
                .inner_size(48.0, 48.0)
                .position(160.0, 50.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .shadow(false)
                .build()
                .unwrap();

            disable_double_click_for_window(&ball_window);

            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::TrayIconBuilder;

            let open_item = MenuItemBuilder::with_id("open", "打开笔记")
                .build(&handle)?;
            let mini_item = MenuItemBuilder::with_id("mini", "缩小悬浮球")
                .build(&handle)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出软件")
                .build(&handle)?;

            let menu = MenuBuilder::new(&handle)
                .item(&open_item)
                .item(&mini_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let handle_clone = handle.clone();
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(handle.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("清枫速记")
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "open" => {
                            if let Some(ball) = app.get_webview_window("ball") {
                                ball.hide().ok();
                            }
                            if let Some(main) = app.get_webview_window("main") {
                                main.show().ok();
                                main.set_focus().ok();
                            }
                        }
                        "mini" => {
                            if let Some(main) = app.get_webview_window("main") {
                                if let Ok(pos) = main.outer_position() {
                                    main.hide().ok();
                                    if let Some(ball) = app.get_webview_window("ball") {
                                        ball.set_position(tauri::Position::Physical(
                                            tauri::PhysicalPosition::new(
                                                pos.x + 140,
                                                pos.y,
                                            ),
                                        ))
                                        .ok();
                                        ball.show().ok();
                                        ball.set_focus().ok();
                                    }
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(main) = app.get_webview_window("main") {
                            if main.is_visible().unwrap_or(false) {
                                if let Ok(pos) = main.outer_position() {
                                    main.hide().ok();
                                    if let Some(ball) = app.get_webview_window("ball") {
                                        ball.set_position(tauri::Position::Physical(
                                            tauri::PhysicalPosition::new(
                                                pos.x + 140,
                                                pos.y,
                                            ),
                                        ))
                                        .ok();
                                        ball.show().ok();
                                        ball.set_focus().ok();
                                    }
                                }
                            } else {
                                if let Some(ball) = app.get_webview_window("ball") {
                                    ball.hide().ok();
                                }
                                main.show().ok();
                                main.set_focus().ok();
                            }
                        }
                    }
                })
                .build(&handle)?;

            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            let handle_for_focus = handle.clone();
            let gs_handle = handle.clone();
            if let Some(main) = handle.get_webview_window("main") {
                let app_handle = handle_for_focus.clone();
                let gs_app = gs_handle.clone();
                let app_exit = handle_for_focus.clone();
                main.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(focused) => {
                            if *focused {
                                keep_top(&app_handle);
                                let gs = gs_app.global_shortcut();
                                let _ = gs.register("PrintScreen");
                            } else {
                                let gs = gs_app.global_shortcut();
                                let _ = gs.unregister("PrintScreen");
                            }
                        }
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            
                            
                            api.prevent_close();
                            let h = app_handle.clone();
                            let exit = app_exit.clone();
                            let _ = h.emit("request-save-exit", ());
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(1500));
                                exit.exit(0);
                            });
                        }
                        _ => {}
                    }
                });
            }

            
            let handle_for_ball = handle.clone();
            let gs_handle_ball = handle.clone();
            if let Some(ball) = handle.get_webview_window("ball") {
                let gs_app = gs_handle_ball.clone();
                ball.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        if *focused {
                            let gs = gs_app.global_shortcut();
                            let _ = gs.register("PrintScreen");
                        } else {
                            let gs = gs_app.global_shortcut();
                            let _ = gs.unregister("PrintScreen");
                        }
                    }
                });
            }

            
            let prtsc_handle = handle.clone();
            let _ = handle.global_shortcut().on_shortcut("PrintScreen", move |app, _shortcut, _event| {
                app.emit("trigger-screenshot", ()).ok();
            });

            keep_top(&handle_clone);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
