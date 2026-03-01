// В этом модуле лежат платформенные детали.
//
// На старте здесь была попытка принудительно переключать консоль в UTF-8.
// Но в старом Windows PowerShell / conhost это может вести себя нестабильно:
// на некоторых машинах начинают ломаться шрифты и кириллица.
//
// Поэтому для интерактивной консоли используем более надежный путь:
// - если stdin/stdout действительно подключены к Windows Console,
//   используем WinAPI-функции `ReadConsoleW` и `WriteConsoleW`;
// - если ввод/вывод перенаправлены, спокойно падаем обратно в обычный stdio.
//
// Так мы одновременно поддерживаем:
// - нормальный интерактивный русский ввод в консоли;
// - обычные редиректы и автоматические smoke tests.

use std::io::{self, Write};

use crate::shutdown;

#[cfg(windows)]
use std::env;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(n_std_handle: u32) -> isize;
    fn GetConsoleMode(console_handle: isize, mode: *mut u32) -> i32;
    fn ReadConsoleW(
        console_input: isize,
        buffer: *mut u16,
        chars_to_read: u32,
        chars_read: *mut u32,
        input_control: *mut core::ffi::c_void,
    ) -> i32;
    fn WriteConsoleW(
        console_output: isize,
        buffer: *const u16,
        chars_to_write: u32,
        chars_written: *mut u32,
        reserved: *mut core::ffi::c_void,
    ) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct ConsoleReadConsoleControl {
    n_length: u32,
    n_initial_chars: u32,
    dw_ctrl_wakeup_mask: u32,
    dw_control_key_state: u32,
}

#[cfg(windows)]
pub fn enable_utf8_console() -> Result<(), String> {
    // Намеренно ничего не меняем в глобальном состоянии консоли.
    //
    // В интерактивном режиме Unicode обеспечивается через
    // `ReadConsoleW` / `WriteConsoleW`, а не через смену code page.
    //
    // Это снижает риск багов старого host-консоли Windows.
    Ok(())
}

#[cfg(windows)]
pub fn startup_warnings() -> Vec<&'static str> {
    // Если переменной `WT_SESSION` нет, почти наверняка мы запущены
    // не в Windows Terminal, а в классическом host-консоли Windows.
    //
    // Именно в таком режиме чаще всего всплывают проблемы со шрифтом
    // и отображением кириллицы.
    if env::var_os("WT_SESSION").is_none() {
        return vec![
            "WARNING: THIS CONSOLE MAY NOT DISPLAY CYRILLIC CORRECTLY.",
            "IF YOU SEE QUESTION MARKS INSTEAD OF RUSSIAN TEXT, CHANGE THE CONSOLE FONT.",
            "IF CONSOLAS IS UNSTABLE, TRY LUCIDA CONSOLE.",
            "RECOMMENDED FONTS: LUCIDA CONSOLE OR CONSOLAS.",
            "RECOMMENDED HOST: WINDOWS TERMINAL.",
        ];
    }

    Vec::new()
}

#[cfg(windows)]
const STD_INPUT_HANDLE: u32 = -10_i32 as u32;

#[cfg(windows)]
const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;

#[cfg(windows)]
const CTRL_C_WAKE_MASK: u32 = 1 << 3;

#[cfg(windows)]
fn get_std_handle(kind: u32) -> Result<isize, String> {
    let handle = unsafe { GetStdHandle(kind) };

    if handle == 0 || handle == -1 {
        return Err(format!("GetStdHandle({kind}) вернул невалидный handle"));
    }

    Ok(handle)
}

#[cfg(windows)]
fn is_console_handle(handle: isize) -> bool {
    let mut mode = 0_u32;
    unsafe { GetConsoleMode(handle, &mut mode) != 0 }
}

#[cfg(windows)]
fn write_console_wide(text: &str, add_newline: bool) -> Result<(), String> {
    let handle = get_std_handle(STD_OUTPUT_HANDLE)?;

    if !is_console_handle(handle) {
        if add_newline {
            println!("{text}");
        } else {
            print!("{text}");
            io::stdout()
                .flush()
                .map_err(|error| format!("Не удалось сбросить stdout: {error}"))?;
        }

        return Ok(());
    }

    let mut utf16: Vec<u16> = text.encode_utf16().collect();

    if add_newline {
        utf16.extend("\r\n".encode_utf16());
    }

    let mut written = 0_u32;
    let result = unsafe {
        WriteConsoleW(
            handle,
            utf16.as_ptr(),
            utf16.len() as u32,
            &mut written,
            std::ptr::null_mut(),
        )
    };

    if result == 0 {
        return Err("WriteConsoleW завершился неудачно".to_string());
    }

    Ok(())
}

#[cfg(windows)]
pub fn print_text(text: &str) -> Result<(), String> {
    write_console_wide(text, false)
}

#[cfg(windows)]
pub fn print_line(text: &str) -> Result<(), String> {
    write_console_wide(text, true)
}

#[cfg(windows)]
pub fn read_line() -> Result<Option<String>, String> {
    if shutdown::is_requested() {
        return Ok(None);
    }

    let handle = get_std_handle(STD_INPUT_HANDLE)?;

    if !is_console_handle(handle) {
        let mut buffer = String::new();
        let bytes_read = match io::stdin().read_line(&mut buffer) {
            Ok(bytes_read) => bytes_read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted && shutdown::is_requested() => {
                return Ok(None);
            }
            Err(error) => {
                return Err(format!("Не удалось прочитать строку из stdin: {error}"));
            }
        };

        if bytes_read == 0 {
            return Ok(None);
        }

        return Ok(Some(buffer));
    }

    let mut input_control = ConsoleReadConsoleControl {
        n_length: std::mem::size_of::<ConsoleReadConsoleControl>() as u32,
        n_initial_chars: 0,
        dw_ctrl_wakeup_mask: CTRL_C_WAKE_MASK,
        dw_control_key_state: 0,
    };

    // Для интерактивной консоли читаем UTF-16 напрямую через WinAPI.
    // Буфер небольшой, но этого достаточно для командного режима.
    let mut buffer = vec![0_u16; 256];
    let mut chars_read = 0_u32;

    let output_handle = get_std_handle(STD_OUTPUT_HANDLE)?;
    let input_control_ptr = if is_console_handle(output_handle) {
        &mut input_control as *mut ConsoleReadConsoleControl as *mut core::ffi::c_void
    } else {
        std::ptr::null_mut()
    };

    let result = unsafe {
        ReadConsoleW(
            handle,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            &mut chars_read,
            input_control_ptr,
        )
    };

    if result == 0 {
        if shutdown::is_requested() {
            return Ok(None);
        }

        return Err("ReadConsoleW завершился неудачно".to_string());
    }

    if chars_read == 0 {
        return Ok(None);
    }

    if shutdown::is_requested() {
        return Ok(None);
    }

    let text = String::from_utf16(&buffer[..chars_read as usize])
        .map_err(|error| format!("Не удалось декодировать UTF-16 ввод: {error}"))?;

    Ok(Some(text))
}

#[cfg(not(windows))]
pub fn enable_utf8_console() -> Result<(), String> {
    // На не-Windows системах отдельная настройка не нужна.
    Ok(())
}

#[cfg(not(windows))]
pub fn startup_warnings() -> Vec<&'static str> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn print_text(text: &str) -> Result<(), String> {
    print!("{text}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Не удалось сбросить stdout: {error}"))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn print_line(text: &str) -> Result<(), String> {
    println!("{text}");
    Ok(())
}

#[cfg(not(windows))]
pub fn read_line() -> Result<Option<String>, String> {
    if shutdown::is_requested() {
        return Ok(None);
    }

    let mut buffer = String::new();
    let bytes_read = match io::stdin().read_line(&mut buffer) {
        Ok(bytes_read) => bytes_read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted && shutdown::is_requested() => {
            return Ok(None);
        }
        Err(error) => {
            return Err(format!("Не удалось прочитать строку из stdin: {error}"));
        }
    };

    if bytes_read == 0 {
        return Ok(None);
    }

    Ok(Some(buffer))
}
