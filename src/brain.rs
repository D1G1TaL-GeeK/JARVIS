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
