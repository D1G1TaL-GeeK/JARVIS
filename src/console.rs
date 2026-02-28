use crate::interfaces::{Informer, Listener};
use crate::platform;
use crate::types::{ReplyKind, UserRequest};

// Этот модуль отвечает за консольный ввод/вывод.
//
// В будущем его можно будет заменить на:
// - микрофонный listener;
// - TTS informer;
// - логирование в файл;
// - потоковый вывод статуса.

pub struct ConsoleListener;

impl ConsoleListener {
    pub fn new() -> Self {
        Self
    }
}

impl Listener for ConsoleListener {
    fn listen(&mut self) -> Result<Option<UserRequest>, String> {
        // Явный prompt помогает видеть границу между запросами.
        platform::print_text("you> ")?;

        let Some(buffer) = platform::read_line()? else {
            return Ok(None);
        };

        Ok(Some(UserRequest::new(buffer)))
    }
}

pub struct ConsoleInformer {
    assistant_name: String,
}

impl ConsoleInformer {
    pub fn new(assistant_name: impl Into<String>) -> Self {
        Self {
            assistant_name: assistant_name.into(),
        }
    }

    fn prefix_for(kind: ReplyKind) -> &'static str {
        match kind {
            ReplyKind::Info => "info",
            ReplyKind::Suggestion => "suggest",
            ReplyKind::Execution => "exec",
            ReplyKind::Error => "error",
        }
    }
}

impl Informer for ConsoleInformer {
    fn inform(&mut self, kind: ReplyKind, message: &str) -> Result<(), String> {
        // Убираем хвостовые переводы строк, чтобы вывод не "разъезжался".
        let clean_message = message.trim_end();
        let prefix = Self::prefix_for(kind);

        platform::print_line(&format!(
            "{}[{prefix}]> {clean_message}",
            self.assistant_name
        ))
    }
}
