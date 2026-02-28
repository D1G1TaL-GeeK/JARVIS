// Подключаем модули текущего crate.
// В маленьком Rust-проекте так обычно и начинают:
// один бинарник и несколько файлов с логически разнесенной ответственностью.
mod assistant;
mod brain;
mod console;
mod executor;
mod interfaces;
mod platform;
mod types;

use assistant::Assistant;
use brain::RuleBasedBrain;
use console::{ConsoleInformer, ConsoleListener};
use executor::LocalExecutor;

fn main() {
    // Для русскоязычного интерфейса на Windows это особенно важно.
    // Без явного перевода консоли в UTF-8 ввод и вывод могут превратиться в `????`.
    if let Err(error) = platform::enable_utf8_console() {
        eprintln!("Предупреждение: не удалось включить UTF-8 для консоли: {error}");
    }

    // Печатаем предупреждения только ASCII-символами,
    // чтобы они были читаемы даже в "плохой" консоли.
    for warning in platform::startup_warnings() {
        if let Err(error) = platform::print_line(&format!("JARVIS[WARN]> {warning}")) {
            eprintln!("JARVIS[WARN]> {warning}");
            eprintln!("Warning output fallback failed: {error}");
        }
    }

    // Создаем конкретные реализации наших "интерфейсов".
    //
    // Аналогия с C++:
    // - `ConsoleListener` похож на класс, который умеет читать ввод;
    // - `RuleBasedBrain` похож на простейший модуль принятия решений;
    // - `LocalExecutor` исполняет безопасные локальные действия;
    // - `ConsoleInformer` выводит ответы пользователю.
    let listener = Box::new(ConsoleListener::new());
    let brain = Box::new(RuleBasedBrain::new("JARVIS"));
    let executor = Box::new(LocalExecutor::new());
    let informer = Box::new(ConsoleInformer::new("JARVIS"));

    // Собираем все части в один объект ассистента.
    let mut assistant = Assistant::new(listener, brain, executor, informer);

    // В Rust функция `main` тоже может завершиться ошибкой,
    // но здесь я показываю максимально явный вариант:
    // если что-то пошло не так, печатаем ошибку и выходим с кодом 1.
    if let Err(error) = assistant.run() {
        eprintln!("JARVIS остановлен с ошибкой: {error}");
        std::process::exit(1);
    }
}
