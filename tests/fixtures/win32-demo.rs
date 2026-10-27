//! Genuine PE executable exercising Win32 APIs inside Wine.
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(
        window: *mut std::ffi::c_void,
        text: *const u16,
        title: *const u16,
        kind: u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcessId() -> u32;
    fn GetTickCount64() -> u64;
}
fn main() {
    let pid = unsafe { GetCurrentProcessId() };
    let ticks = unsafe { GetTickCount64() };
    let message = format!("WINCRAFT_WIN32_OK pid={pid} ticks={ticks}");
    println!("{message}");
    if std::env::args().any(|a| a == "--smoke") {
        std::fs::write(r"Z:\tmp\wincraft-win32-ok.txt", &message).expect("Wine Unix drive mapping");
        return;
    }
    let text: Vec<u16> = format!("This is a Windows EXE running on WinCraft OS.\n\n{message}\n\nuser32.dll + kernel32.dll supplied by Wine.").encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "WinCraft / Win32 demo"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x40);
    }
}
