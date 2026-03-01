use crate::interfaces::Brain;
use crate::types::{Action, Decision, UserRequest};

// `RuleBasedBrain` — это намеренно очень простой мозг ассистента.
//
// Здесь нет LLM, embeddings и сложного NLP.
// Зато есть важная польза:
// - понятная логика;
// - быстрый запуск;
// - хорошая база для изучения Rust;
// - правильная точка расширения под локальные модели в будущем.

pub struct RuleBasedBrain {
    assistant_name: String,
}

impl RuleBasedBrain {
    pub fn new(assistant_name: impl Into<String>) -> Self {
        Self {
            assistant_name: assistant_name.into(),
        }
    }

    fn help_message(&self) -> String {
        format!(
            "{name} пока работает в учебном текстовом режиме.\n\
Доступные команды:\n\
- помощь\n\
- который час\n\
- покажи файлы\n\
- слушай <секунды>\n\
- запиши <секунды>\n\
- прослушай запись\n\
- проиграй запись\n\
- повтори <текст>\n\
- предложи команду\n\
- выход",
            name = self.assistant_name
        )
    }

    fn build_suggestion(&self, text: &str) -> String {
        if text.contains("файл") || text.contains("папк") {
            return "Предлагаю безопасную команду: `покажи файлы`.".to_string();
        }

        if text.contains("врем") || text.contains("час") {
            return "Предлагаю команду: `который час`.".to_string();
        }

        if text.contains("повтор") || text.contains("скажи") {
            return "Попробуй команду: `повтори привет, я изучаю Rust`.".to_string();
        }

        if text.contains("слуш") || text.contains("запиш") || text.contains("микрофон") {
            return "Попробуй голосовую команду: `слушай 5`.".to_string();
        }

        if text.contains("запись") || text.contains("проигр") || text.contains("прослуш") {
            return "После записи можно ввести `прослушай запись`.".to_string();
        }

        "Можно начать с одной из команд: `который час`, `покажи файлы`, `повтори привет`, `выход`."
            .to_string()
    }

    fn extract_tail<'a>(&self, text: &'a str, prefixes: &[&str]) -> Option<&'a str> {
        for prefix in prefixes {
            if let Some(rest) = text.strip_prefix(prefix) {
                let trimmed = rest.trim();

                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        None
    }

    fn parse_recording_duration(&self, text: &str) -> Result<Option<u32>, String> {
        // На этом шаге поддерживаем очень простой формат:
        // `слушай 5` или `запиши 5`.
        //
        // Так проще понять и парсер, и `match` в executor,
        // чем сразу строить сложный CLI-язык.
        let mut parts = text.split_whitespace();
        let Some(command) = parts.next() else {
            return Ok(None);
        };

        if command != "слушай" && command != "запиши" && command != "record" {
            return Ok(None);
        }

        let Some(seconds_text) = parts.next() else {
            return Err(
                "Для записи укажи длительность, например: `слушай 5` или `запиши 5`."
                    .to_string(),
            );
        };

        if parts.next().is_some() {
            return Err(
                "Пока поддерживается только формат `слушай <секунды>`, например `слушай 5`."
                    .to_string(),
            );
        }

        let duration_secs = seconds_text.parse::<u32>().map_err(|_| {
            "Не удалось понять длительность записи. Пример правильной команды: `слушай 5`."
                .to_string()
        })?;

        if duration_secs == 0 {
            return Err("Длительность записи должна быть больше нуля.".to_string());
        }

        if duration_secs > 30 {
            return Err(
                "Для первого шага ограничим запись 30 секундами. Попробуй, например, `слушай 5`."
                    .to_string(),
            );
        }

        Ok(Some(duration_secs))
    }

    fn is_play_last_recording_command(&self, text: &str) -> bool {
        // Не смешиваем эту команду с `слушай 5`.
        // Здесь нас интересуют только явные формулировки воспроизведения записи.
        let patterns = [
            "прослушай запись",
            "проиграй запись",
            "воспроизведи запись",
            "проиграй последнюю запись",
            "воспроизведи последнюю запись",
            "play recording",
        ];

        patterns.iter().any(|pattern| text == *pattern)
    }
}

impl Brain for RuleBasedBrain {
    fn think(&mut self, request: &UserRequest) -> Decision {
        // Пока используем нормализованный текст для простого роутинга.
        // Это быстрый и очень дешевый способ понять базовые команды.
        let text = request.normalized_text.as_str();

        if request.is_empty() {
            return Decision::inform("Я ничего не получил. Напиши команду или `помощь`.");
        }

        if text == "помощь" || text == "help" || text.contains("что умеешь") {
            return Decision::inform(self.help_message());
        }

        if text == "выход" || text == "exit" || text == "quit" {
            return Decision::exit("Завершаю работу. До связи.");
        }

        if text.contains("который час") || text == "время" || text.contains("сколько времени")
        {
            return Decision::execute("Смотрю локальное время.", Action::ShowLocalTime);
        }

        if text.contains("покажи файлы")
            || text.contains("список файлов")
            || text.contains("что в папке")
        {
            return Decision::execute(
                "Считываю содержимое текущей директории.",
                Action::ListCurrentDirectory,
            );
        }

        match self.parse_recording_duration(text) {
            Ok(Some(duration_secs)) => {
                return Decision::execute(
                    format!(
                        "Начинаю запись с микрофона на {duration_secs} сек. Говори сразу после этой строки."
                    ),
                    Action::RecordMicrophoneClip { duration_secs },
                );
            }
            Ok(None) => {}
            Err(error) => return Decision::inform(error),
        }

        if self.is_play_last_recording_command(text) {
            return Decision::execute(
                "Пробую воспроизвести последнюю сохраненную запись.",
                Action::PlayLastRecording,
            );
        }

        if let Some(phrase) = self.extract_tail(text, &["повтори ", "скажи "]) {
            return Decision::execute(
                "Исполняю локальное действие `RepeatText`.",
                Action::RepeatText(phrase.to_string()),
            );
        }

        if text.starts_with("предложи") || text.starts_with("suggest") {
            return Decision::suggest(self.build_suggestion(text));
        }

        // Здесь удобно показать, что у нас сохранился и исходный текст.
        // Это пригодится позже, когда будет LLM и нормальные логи.
        Decision::suggest(format!(
            "Я пока не понял запрос `{}`. Начни с `помощь` или попроси: `предложи команду`.",
            request.original_text
        ))
    }
}
