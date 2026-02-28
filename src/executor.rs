use std::env;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::interfaces::Executor;
use crate::types::Action;

// Executor — место, где мы превращаем абстрактное решение
// в конкретное локальное действие.
//
// Сейчас действия только безопасные и демонстрационные.
// Это сделано специально:
// - проект остается понятным;
// - меньше риска случайно выполнить что-то опасное;
// - позже сюда можно добавить белый список системных инструментов.

pub struct LocalExecutor {
    max_visible_entries: usize,
}

impl LocalExecutor {
    pub fn new() -> Self {
        Self {
            max_visible_entries: 12,
        }
    }

    fn local_time_string(&self) -> String {
        // Стандартная библиотека Rust умеет хранить время,
        // но не очень удобно форматирует локальное "часы:минуты:секунды"
        // без дополнительных crate.
        //
        // Чтобы не тянуть зависимости на первом шаге,
        // используем локальный PowerShell как системный источник времени.
        // Если вдруг это не сработает, даем понятный запасной вариант.
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Date -Format HH:mm:ss"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

                if !text.is_empty() {
                    return text;
                }
            }
        }

        // Запасной вариант без красивого локального форматирования.
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => format!(
                "не удалось красиво получить локальное время, UNIX={}",
                duration.as_secs()
            ),
            Err(_) => "не удалось получить текущее время".to_string(),
        }
    }

    fn list_current_directory(&self) -> Result<String, String> {
        let current_dir = env::current_dir()
            .map_err(|error| format!("Не удалось получить текущую директорию: {error}"))?;

        let mut entries = Vec::new();

        for entry in fs::read_dir(&current_dir).map_err(|error| {
            format!(
                "Не удалось прочитать директорию {}: {error}",
                current_dir.display()
            )
        })? {
            let entry =
                entry.map_err(|error| format!("Не удалось прочитать запись каталога: {error}"))?;

            let metadata = entry
                .metadata()
                .map_err(|error| format!("Не удалось получить metadata: {error}"))?;

            let name = entry.file_name().to_string_lossy().into_owned();

            // Добавляем краткую метку, чтобы сразу видеть тип объекта.
            let marked_name = if metadata.is_dir() {
                format!("[DIR]  {name}")
            } else {
                format!("[FILE] {name}")
            };

            entries.push(marked_name);
        }

        entries.sort();

        if entries.is_empty() {
            return Ok(format!("Папка {} пуста.", current_dir.display()));
        }

        let total = entries.len();
        let visible_entries: Vec<String> = entries
            .iter()
            .take(self.max_visible_entries)
            .cloned()
            .collect();

        let mut message = format!(
            "Папка {} содержит {} объектов:\n{}",
            current_dir.display(),
            total,
            visible_entries.join("\n")
        );

        if total > self.max_visible_entries {
            message.push_str(&format!(
                "\n... и еще {} объектов не показаны, чтобы не захламлять вывод.",
                total - self.max_visible_entries
            ));
        }

        Ok(message)
    }
}

impl Executor for LocalExecutor {
    fn execute(&mut self, action: &Action) -> Result<String, String> {
        match action {
            Action::ShowLocalTime => Ok(format!("Сейчас {}.", self.local_time_string())),
            Action::ListCurrentDirectory => self.list_current_directory(),
            Action::RepeatText(text) => Ok(format!("Повторяю: {text}")),
        }
    }
}
