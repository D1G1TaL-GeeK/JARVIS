use crate::types::{Action, Decision, ReplyKind, UserRequest};

// Этот файл содержит только абстракции.
//
// В Rust для такой роли обычно используют `trait`.
// Если сравнивать с C++, то это близко к интерфейсам или
// абстрактным базовым классам с виртуальными методами.

pub trait Listener {
    // `Option<UserRequest>` означает:
    // - `Some(request)` -> пользователь что-то ввел;
    // - `None` -> входной поток закрыт, можно завершаться.
    fn listen(&mut self) -> Result<Option<UserRequest>, String>;
}

pub trait Brain {
    // Brain получает уже распознанный запрос и решает,
    // что ассистент должен ответить или сделать.
    fn think(&mut self, request: &UserRequest) -> Decision;
}

pub trait Executor {
    // Executor исполняет только уже разрешенное действие.
    // Это важно: Brain решает "что делать",
    // а Executor отвечает за "как именно это сделать".
    fn execute(&mut self, action: &Action) -> Result<String, String>;

    // Отдельный hook на завершение полезен для cleanup-вещей:
    // временных файлов, кэша, открытых ресурсов и т.д.
    //
    // Здесь есть default-реализация, чтобы не заставлять каждый executor
    // обязательно что-то делать на shutdown.
    fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub trait Informer {
    // Informer отвечает за последний шаг конвейера:
    // показать или озвучить ответ.
    //
    // Сейчас это простой вывод в консоль.
    // Позже здесь можно будет подключить TTS.
    fn inform(&mut self, kind: ReplyKind, message: &str) -> Result<(), String>;
}
