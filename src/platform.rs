// В этом модуле лежат платформенные детали.
//
// Пока нам нужна только одна вещь:
// корректно включить UTF-8 в консоли Windows, чтобы русский текст
// нормально проходил через stdin/stdout.
//
// Это особенно актуально для GNU-сборки Rust на Windows,
// где кодировка консоли иначе часто становится источником проблем.

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleCP(code_page: u32) -> i32;
    fn SetConsoleOutputCP(code_page: u32) -> i32;
}

#[cfg(windows)]
pub fn enable_utf8_console() -> Result<(), String> {
    const UTF8_CODE_PAGE: u32 = 65001;

    // `unsafe` здесь нужен потому, что мы вызываем WinAPI напрямую.
    // Это нормальная ситуация для низкоуровневой платформенной интеграции.
    let input_result = unsafe { SetConsoleCP(UTF8_CODE_PAGE) };
    if input_result == 0 {
        return Err("SetConsoleCP вернул 0".to_string());
    }

    let output_result = unsafe { SetConsoleOutputCP(UTF8_CODE_PAGE) };
    if output_result == 0 {
        return Err("SetConsoleOutputCP вернул 0".to_string());
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn enable_utf8_console() -> Result<(), String> {
    // На не-Windows системах отдельная настройка не нужна.
    Ok(())
}
