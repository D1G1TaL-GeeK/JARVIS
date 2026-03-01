// Здесь лежат общие типы данных, которыми обмениваются модули.
//
// Отдельный файл с типами полезен по двум причинам:
// 1. меньше циклических зависимостей между модулями;
// 2. проще понять "язык", на котором общаются части системы.

#[derive(Debug, Clone)]
pub struct UserRequest {
    // Исходный текст полезно сохранять,
    // потому что позже он может понадобиться для логов
    // или для более "умного" разбора.
    pub original_text: String,

    // Нормализованный текст удобен для простых сравнений:
    // убираем лишние пробелы и приводим все к нижнему регистру.
    pub normalized_text: String,
}

impl UserRequest {
    pub fn new(text: String) -> Self {
        // На Windows первая строка иногда может прийти с BOM (`\u{feff}`),
        // особенно при редиректе ввода.
        // Снаружи это выглядит как "невидимый символ", который ломает сравнение.
        let cleaned_text = text.trim().trim_start_matches('\u{feff}');
        let normalized_text = cleaned_text.to_lowercase();

        Self {
            original_text: cleaned_text.to_string(),
            normalized_text,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.normalized_text.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ReplyKind {
    // Обычное информационное сообщение.
    Info,

    // Ассистент что-то предлагает, но не исполняет.
    Suggestion,

    // Ассистент сообщает о результате исполнения действия.
    Execution,

    // Ошибка отделена в отдельный тип,
    // чтобы потом ее было легче красиво отрисовывать или озвучивать.
    Error,
}

#[derive(Debug, Clone)]
pub enum Action {
    // Безопасное локальное действие: показать текущее время.
    ShowLocalTime,

    // Еще одно безопасное действие: показать содержимое текущей папки.
    ListCurrentDirectory,

    // Временный debug-инструмент:
    // проиграть последнюю сохраненную запись с микрофона.
    PlayLastRecording,

    // Первый настоящий "голосовой" шаг:
    // записать короткий фрагмент с микрофона в WAV-файл.
    //
    // Здесь мы сознательно храним только длительность записи.
    // Остальные детали — выбор устройства, sample rate, путь к файлу —
    // определяются уже на этапе исполнения.
    RecordMicrophoneClip {
        duration_secs: u32,
    },

    // Простейшее "исполнение" с полезной нагрузкой:
    // ассистент повторяет переданный текст.
    RepeatText(String),
}

#[derive(Debug, Clone)]
pub struct Decision {
    // В каком режиме отвечаем: просто информируем,
    // предлагаем или показываем результат исполнения.
    pub kind: ReplyKind,

    // Текстовое сообщение для пользователя.
    pub message: String,

    // Если действие `Some(...)`, значит после сообщения
    // нужно еще вызвать Executor.
    pub action: Option<Action>,

    // Этот флаг завершает главный цикл.
    pub should_exit: bool,
}

impl Decision {
    pub fn inform(message: impl Into<String>) -> Self {
        Self {
            kind: ReplyKind::Info,
            message: message.into(),
            action: None,
            should_exit: false,
        }
    }

    pub fn suggest(message: impl Into<String>) -> Self {
        Self {
            kind: ReplyKind::Suggestion,
            message: message.into(),
            action: None,
            should_exit: false,
        }
    }

    pub fn execute(message: impl Into<String>, action: Action) -> Self {
        Self {
            kind: ReplyKind::Info,
            message: message.into(),
            action: Some(action),
            should_exit: false,
        }
    }

    pub fn exit(message: impl Into<String>) -> Self {
        Self {
            kind: ReplyKind::Info,
            message: message.into(),
            action: None,
            should_exit: true,
        }
    }
}
