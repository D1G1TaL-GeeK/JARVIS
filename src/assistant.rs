use crate::interfaces::{Brain, Executor, Informer, Listener};
use crate::shutdown;
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
            "JARVIS запущен. Уже доступны текстовые команды, запись микрофона и debug-воспроизведение последней записи. Напиши `помощь` или попробуй `слушай 5`.",
        )?;

        let run_result = self.run_loop();
        let shutdown_result = self.executor.shutdown();

        match (run_result, shutdown_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(run_error), Ok(())) => Err(run_error),
            (Ok(()), Err(shutdown_error)) => Err(format!(
                "JARVIS завершил работу, но cleanup временных записей завершился ошибкой: {shutdown_error}"
            )),
            (Err(run_error), Err(shutdown_error)) => Err(format!(
                "{run_error}\nДополнительно cleanup временных записей завершился ошибкой: {shutdown_error}"
            )),
        }
    }

    fn run_loop(&mut self) -> Result<(), String> {
        loop {
            if self.handle_shutdown_request()? {
                break;
            }

            let maybe_request = self.listener.listen()?;

            if self.handle_shutdown_request()? {
                break;
            }

            let Some(request) = maybe_request else {
                if self.handle_shutdown_request()? {
                    break;
                }

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
                        if self.handle_shutdown_request()? {
                            break;
                        }

                        self.informer
                            .inform(ReplyKind::Execution, &execution_report)?;
                    }
                    Err(error) => {
                        if self.handle_shutdown_request()? {
                            break;
                        }

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

    fn handle_shutdown_request(&mut self) -> Result<bool, String> {
        if shutdown::is_requested() {
            self.informer
                .inform(ReplyKind::Info, "Получен Ctrl+C. Завершаю работу.")?;
            return Ok(true);
        }

        Ok(false)
    }
}
