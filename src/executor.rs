use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audio::MicrophoneRecorder;
use crate::interfaces::Executor;
use crate::shutdown;
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
    microphone_recorder: MicrophoneRecorder,
    last_recording_path: Option<PathBuf>,
}

impl LocalExecutor {
    pub fn new() -> Self {
        Self {
            max_visible_entries: 12,
            microphone_recorder: MicrophoneRecorder::new(),
            last_recording_path: None,
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

    fn record_microphone_clip(&mut self, duration_secs: u32) -> Result<String, String> {
        // На этом шаге мы сознательно не делаем потоковую обработку,
        // VAD или распознавание речи.
        //
        // Цель проще:
        // - взять звук с микрофона;
        // - положить его в обычный WAV;
        // - показать пользователю, что "listen" часть конвейера уже реальна.
        let summary = self.microphone_recorder.record_clip(duration_secs)?;
        self.last_recording_path = Some(summary.output_path.clone());

        Ok(format!(
            "Запись завершена.\n\
Устройство: {}\n\
Файл: {}\n\
Формат: {} Hz, {} канал(а), 16-bit PCM\n\
Сэмплов записано: {}\n\
Примерная длительность: {:.2} сек.\n\
Чтобы прослушать последнюю запись, введи `прослушай запись`.",
            summary.device_name,
            summary.output_path.display(),
            summary.sample_rate,
            summary.channels,
            summary.captured_samples,
            summary.approx_duration_secs
        ))
    }

    fn play_last_recording(&self) -> Result<String, String> {
        let Some(path) = self.last_recording_path.as_ref() else {
            return Err(
                "Пока нет последней записи для воспроизведения. Сначала выполни `слушай 5`."
                    .to_string(),
            );
        };

        self.play_wav_file(path)?;

        Ok(format!(
            "Воспроизведение завершено.\nФайл: {}",
            path.display()
        ))
    }

    fn play_wav_file(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!(
                "Файл последней записи не найден: {}",
                path.display()
            ));
        }

        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .map_err(|error| format!("Не удалось получить текущую директорию: {error}"))?
                .join(path)
        };

        let escaped_path = full_path
            .to_string_lossy()
            // Одинарные кавычки внутри PowerShell-строки нужно дублировать.
            .replace('\'', "''");

        // Для временного debug-режима используем встроенный в Windows
        // `System.Media.SoundPlayer`.
        //
        // Это не идеальный финальный аудио-стек, но отличный практический
        // компромисс для текущего этапа:
        // - без новых зависимостей;
        // - работает локально;
        // - синхронно воспроизводит WAV и дает быстрый feedback.
        let script = [
            &format!("$path = '{escaped_path}'"),
            "$player = New-Object System.Media.SoundPlayer $path",
            "$player.Load()",
            "$player.PlaySync()",
        ]
        .join("; ");

        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Не удалось запустить PowerShell для воспроизведения: {error}"))?;

        loop {
            if shutdown::is_requested() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Воспроизведение было прервано сигналом Ctrl+C.".to_string());
            }

            let Some(status) = child
                .try_wait()
                .map_err(|error| format!("Не удалось дождаться завершения плеера: {error}"))?
            else {
                thread::sleep(std::time::Duration::from_millis(25));
                continue;
            };

            let mut stdout = String::new();
            if let Some(mut stdout_pipe) = child.stdout.take() {
                let _ = stdout_pipe.read_to_string(&mut stdout);
            }

            let mut stderr = String::new();
            if let Some(mut stderr_pipe) = child.stderr.take() {
                let _ = stderr_pipe.read_to_string(&mut stderr);
            }

            if status.success() {
                return Ok(());
            }

            let stderr = stderr.trim().to_string();
            let stdout = stdout.trim().to_string();

            let details = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "PowerShell завершился с ошибкой без текста.".to_string()
            };

            return Err(format!(
                "Не удалось воспроизвести WAV-файл через системный плеер: {details}"
            ));
        }
    }

    fn cleanup_temporary_recordings(&mut self) -> Result<(), String> {
        // Все записи в текущей версии считаются временными debug-артефактами.
        // Поэтому при завершении ассистента просто очищаем всю директорию `recordings`.
        let _deleted_entries = self.microphone_recorder.cleanup_recordings()?;
        self.last_recording_path = None;
        Ok(())
    }
}

impl Executor for LocalExecutor {
    fn execute(&mut self, action: &Action) -> Result<String, String> {
        match action {
            Action::ShowLocalTime => Ok(format!("Сейчас {}.", self.local_time_string())),
            Action::ListCurrentDirectory => self.list_current_directory(),
            Action::RecordMicrophoneClip { duration_secs } => {
                self.record_microphone_clip(*duration_secs)
            }
            Action::PlayLastRecording => self.play_last_recording(),
            Action::RepeatText(text) => Ok(format!("Повторяю: {text}")),
        }
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.cleanup_temporary_recordings()
    }
}
