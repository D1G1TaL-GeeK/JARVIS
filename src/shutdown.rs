use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

// Этот модуль хранит глобальный флаг остановки приложения.
//
// Зачем он нужен:
// - обработчик `Ctrl+C` срабатывает вне основного цикла;
// - ассистент, запись аудио и консольный ввод должны видеть общий сигнал на остановку;
// - cleanup должен пройти по штатному пути, а не через аварийное завершение процесса.

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static HANDLER_INSTALLED: OnceLock<()> = OnceLock::new();

pub fn install_ctrlc_handler() -> Result<(), String> {
    if HANDLER_INSTALLED.get().is_some() {
        return Ok(());
    }

    ctrlc::set_handler(|| {
        request_shutdown();
    })
    .map_err(|error| format!("Не удалось установить обработчик Ctrl+C: {error}"))?;

    let _ = HANDLER_INSTALLED.set(());
    Ok(())
}

pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn is_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}
