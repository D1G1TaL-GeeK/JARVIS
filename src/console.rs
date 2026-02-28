use std::io::{self, Write};

use crate::interfaces::{Informer, Listener};
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
        print!("you> ");

        // `flush()` нужен потому, что без него prompt может остаться в буфере
        // и не показаться пользователю до чтения строки.
        io::stdout()
            .flush()
            .map_err(|error| format!("Не удалось сбросить stdout: {error}"))?;

        let mut buffer = String::new();

        let bytes_read = io::stdin()
            .read_line(&mut buffer)
            .map_err(|error| format!("Не удалось прочитать строку из stdin: {error}"))?;

        // `0` байт означает EOF: поток закрыт.
        // Это нормальная ситуация, а не ошибка.
        if bytes_read == 0 {
            return Ok(None);
        }

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

        println!("{}[{prefix}]> {clean_message}", self.assistant_name);
        Ok(())
    }
}
