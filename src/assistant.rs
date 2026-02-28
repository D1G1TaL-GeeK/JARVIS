use crate::interfaces::{Brain, Executor, Informer, Listener};
use crate::types::ReplyKind;

// `Assistant` — главный оркестратор.
//
// Он сам не "умный" и не "говорящий".
// Его задача — просто правильно соединить этапы конвейера:
//
// 1. listen
// 2. think
// 3. execute
// 4. inform
//
// Такой подход хорош тем, что каждую часть потом можно заменить отдельно.

pub struct Assistant {
    listener: Box<dyn Listener>,
    brain: Box<dyn Brain>,
    executor: Box<dyn Executor>,
    informer: Box<dyn Informer>,
}

impl Assistant {
    pub fn new(
        listener: Box<dyn Listener>,
        brain: Box<dyn Brain>,
        executor: Box<dyn Executor>,
        informer: Box<dyn Informer>,
    ) -> Self {
        Self {
            listener,
            brain,
            executor,
            informer,
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        self.informer.inform(
            ReplyKind::Info,
            "JARVIS запущен. Пока это учебный текстовый режим без микрофона и без LLM. Напиши `помощь`.",
        )?;

        loop {
            let maybe_request = self.listener.listen()?;

            let Some(request) = maybe_request else {
                self.informer
                    .inform(ReplyKind::Info, "Входной поток закрыт. Завершаю работу.")?;
                break;
            };

            let decision = self.brain.think(&request);

            // Если у Brain уже есть сообщение, сначала показываем его.
            // Это полезно, когда нужно объяснить пользователю,
            // что именно сейчас будет исполнено.
            if !decision.message.is_empty() {
                self.informer.inform(decision.kind, &decision.message)?;
            }

            if let Some(action) = decision.action.as_ref() {
                match self.executor.execute(action) {
                    Ok(execution_report) => {
                        self.informer
                            .inform(ReplyKind::Execution, &execution_report)?;
                    }
                    Err(error) => {
                        self.informer.inform(
                            ReplyKind::Error,
                            &format!("Во время исполнения произошла ошибка: {error}"),
                        )?;
                    }
                }
            }

            if decision.should_exit {
                break;
            }
        }

        Ok(())
    }
}
