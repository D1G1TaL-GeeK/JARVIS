use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::shutdown;

// Этот модуль отвечает за самый первый "настоящий" голосовой шаг:
// запись короткого фрагмента с микрофона в обычный WAV-файл.
//
// Важно понимать, чего здесь пока НЕТ:
// - нет распознавания речи;
// - нет VAD (voice activity detection);
// - нет шумоподавления;
// - нет потоковой отправки в LLM.
//
// Это сделано намеренно.
// Сначала нам нужно получить честный и понятный слой "слушать",
// который можно отдельно проверить и понять.

pub struct RecordingSummary {
    pub device_name: String,
    pub output_path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub captured_samples: usize,
    pub approx_duration_secs: f64,
}

pub struct MicrophoneRecorder {
    output_directory: PathBuf,
    max_duration_secs: u32,
    poll_interval: Duration,
    completion_grace_period: Duration,
}

impl MicrophoneRecorder {
    pub fn new() -> Self {
        Self {
            output_directory: PathBuf::from("recordings"),
            max_duration_secs: 30,
            poll_interval: Duration::from_millis(25),
            completion_grace_period: Duration::from_secs(2),
        }
    }

    pub fn record_clip(&self, duration_secs: u32) -> Result<RecordingSummary, String> {
        if duration_secs == 0 {
            return Err("Длительность записи должна быть больше нуля.".to_string());
        }

        if duration_secs > self.max_duration_secs {
            return Err(format!(
                "Для учебного шага ограничим запись {} секундами.",
                self.max_duration_secs
            ));
        }

        fs::create_dir_all(&self.output_directory).map_err(|error| {
            format!(
                "Не удалось создать директорию для записей {}: {error}",
                self.output_directory.display()
            )
        })?;

        let host = cpal::default_host();

        // Берем системное устройство ввода по умолчанию.
        // Для первого шага это лучше, чем сразу строить сложный выбор девайса.
        let device = host.default_input_device().ok_or_else(|| {
            "Не найден микрофон по умолчанию. Проверь, подключен ли микрофон и разрешен ли доступ к нему.".to_string()
        })?;

        let device_name = device
            .description()
            .map(|description| description.name().to_string())
            .unwrap_or_else(|_| "Unknown input device".to_string());

        let supported_config = device.default_input_config().map_err(|error| {
            format!(
                "Не удалось получить конфигурацию микрофона `{device_name}`: {error}"
            )
        })?;

        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let channels = stream_config.channels;
        let sample_rate = stream_config.sample_rate;

        // CPAL отдает сэмплы по одному на каждый канал.
        // Если устройство стерео, порядок данных будет примерно такой:
        // L, R, L, R, L, R ...
        //
        // Поэтому целевое число элементов в буфере = секунды * sample_rate * channels.
        let target_sample_count =
            sample_rate as usize * duration_secs as usize * channels as usize;

        // Для простоты и прозрачности накапливаем запись в памяти,
        // а уже потом одним проходом сохраняем в WAV.
        //
        // Для коротких командных записей это нормально:
        // код проще понять, а памяти уходит немного.
        let shared_samples = Arc::new(Mutex::new(Vec::<i16>::with_capacity(target_sample_count)));
        let stream_finished = Arc::new(AtomicBool::new(false));
        let stream_error = Arc::new(Mutex::new(None::<String>));

        let stream = self.build_input_stream(
            &device,
            &stream_config,
            sample_format,
            target_sample_count,
            Arc::clone(&shared_samples),
            Arc::clone(&stream_finished),
            Arc::clone(&stream_error),
        )?;

        stream.play().map_err(|error| {
            format!("Не удалось запустить поток записи с микрофона `{device_name}`: {error}")
        })?;

        let deadline =
            Instant::now() + Duration::from_secs(duration_secs as u64) + self.completion_grace_period;

        while !stream_finished.load(Ordering::SeqCst) && Instant::now() < deadline {
            if shutdown::is_requested() {
                break;
            }

            thread::sleep(self.poll_interval);
        }

        // После `drop(stream)` callback больше не должен приходить.
        drop(stream);

        if shutdown::is_requested() {
            return Err("Запись была прервана сигналом Ctrl+C.".to_string());
        }

        if let Some(error) = stream_error
            .lock()
            .map_err(|_| "Не удалось прочитать ошибку аудио-потока.".to_string())?
            .clone()
        {
            return Err(format!("Во время записи произошла ошибка аудио-потока: {error}"));
        }

        if !stream_finished.load(Ordering::SeqCst) {
            return Err(
                "Запись не завершилась вовремя. Возможно, микрофон не начал отдавать данные."
                    .to_string(),
            );
        }

        let mut guard = shared_samples
            .lock()
            .map_err(|_| "Не удалось получить записанные аудио-данные.".to_string())?;
        let captured_samples = std::mem::take(&mut *guard);
        drop(guard);

        if captured_samples.is_empty() {
            return Err(
                "Микрофон не вернул ни одного сэмпла. Проверь устройство ввода и права доступа."
                    .to_string(),
            );
        }

        let output_path = self.build_output_path();
        self.write_wav_file(&output_path, sample_rate, channels, &captured_samples)?;

        let approx_duration_secs =
            captured_samples.len() as f64 / sample_rate as f64 / channels as f64;

        Ok(RecordingSummary {
            device_name,
            output_path,
            sample_rate,
            channels,
            captured_samples: captured_samples.len(),
            approx_duration_secs,
        })
    }

    pub fn cleanup_recordings(&self) -> Result<usize, String> {
        let recordings_dir = self.output_directory();

        if !recordings_dir.exists() {
            return Ok(0);
        }

        let deleted_entries = fs::read_dir(recordings_dir)
            .map_err(|error| {
                format!(
                    "Не удалось прочитать директорию временных записей {}: {error}",
                    recordings_dir.display()
                )
            })?
            .count();

        fs::remove_dir_all(recordings_dir).map_err(|error| {
            format!(
                "Не удалось удалить директорию временных записей {}: {error}",
                recordings_dir.display()
            )
        })?;

        Ok(deleted_entries)
    }

    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    fn build_input_stream(
        &self,
        device: &cpal::Device,
        stream_config: &StreamConfig,
        sample_format: SampleFormat,
        target_sample_count: usize,
        shared_samples: Arc<Mutex<Vec<i16>>>,
        stream_finished: Arc<AtomicBool>,
        stream_error: Arc<Mutex<Option<String>>>,
    ) -> Result<Stream, String> {
        match sample_format {
            SampleFormat::I8 => self.build_typed_input_stream::<i8>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::I16 => self.build_typed_input_stream::<i16>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::I32 => self.build_typed_input_stream::<i32>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::I64 => self.build_typed_input_stream::<i64>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::U8 => self.build_typed_input_stream::<u8>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::U16 => self.build_typed_input_stream::<u16>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::U32 => self.build_typed_input_stream::<u32>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::U64 => self.build_typed_input_stream::<u64>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::F32 => self.build_typed_input_stream::<f32>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            SampleFormat::F64 => self.build_typed_input_stream::<f64>(
                device,
                stream_config,
                target_sample_count,
                shared_samples,
                stream_finished,
                stream_error,
            ),
            other => Err(format!(
                "Формат аудио-потока `{other:?}` пока не поддерживается в учебной версии."
            )),
        }
    }

    fn build_typed_input_stream<T>(
        &self,
        device: &cpal::Device,
        stream_config: &StreamConfig,
        target_sample_count: usize,
        shared_samples: Arc<Mutex<Vec<i16>>>,
        stream_finished: Arc<AtomicBool>,
        stream_error: Arc<Mutex<Option<String>>>,
    ) -> Result<Stream, String>
    where
        T: SizedSample,
        i16: FromSample<T>,
    {
        let callback_samples = Arc::clone(&shared_samples);
        let callback_finished = Arc::clone(&stream_finished);
        let callback_error = Arc::clone(&stream_error);

        let error_finished = Arc::clone(&stream_finished);
        let error_slot = Arc::clone(&stream_error);

        device
            .build_input_stream(
                stream_config,
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    if callback_finished.load(Ordering::SeqCst) {
                        return;
                    }

                    let Ok(mut guard) = callback_samples.lock() else {
                        Self::store_stream_error(
                            &callback_error,
                            "Не удалось заблокировать буфер аудио-данных.".to_string(),
                        );
                        callback_finished.store(true, Ordering::SeqCst);
                        return;
                    };

                    let remaining = target_sample_count.saturating_sub(guard.len());
                    if remaining == 0 {
                        callback_finished.store(true, Ordering::SeqCst);
                        return;
                    }

                    // Конвертируем входной тип сэмпла в `i16`.
                    // Это удобный и практичный формат для WAV и будущих STT-этапов.
                    for sample in data.iter().take(remaining) {
                        guard.push(i16::from_sample(*sample));
                    }

                    if guard.len() >= target_sample_count {
                        callback_finished.store(true, Ordering::SeqCst);
                    }
                },
                move |error| {
                    Self::store_stream_error(
                        &error_slot,
                        format!("CPAL stream error: {error}"),
                    );
                    error_finished.store(true, Ordering::SeqCst);
                },
                None,
            )
            .map_err(|error| format!("Не удалось создать поток записи с микрофона: {error}"))
    }

    fn build_output_path(&self) -> PathBuf {
        let unix_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        self.output_directory
            .join(format!("jarvis-recording-{unix_millis}.wav"))
    }

    fn write_wav_file(
        &self,
        output_path: &PathBuf,
        sample_rate: u32,
        channels: u16,
        captured_samples: &[i16],
    ) -> Result<(), String> {
        let wav_spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(output_path, wav_spec).map_err(|error| {
            format!(
                "Не удалось создать WAV-файл {}: {error}",
                output_path.display()
            )
        })?;

        // Используем специализированный `i16` writer.
        // Для этой конкретной задачи он быстрее и проще обычного `write_sample`,
        // потому что формат у нас уже фиксирован: 16-bit PCM.
        {
            let mut sample_writer = writer.get_i16_writer(captured_samples.len() as u32);

            for sample in captured_samples {
                sample_writer.write_sample(*sample);
            }

            sample_writer
                .flush()
                .map_err(|error| format!("Не удалось записать PCM-данные в WAV: {error}"))?;
        }

        writer
            .finalize()
            .map_err(|error| format!("Не удалось завершить WAV-файл: {error}"))?;

        Ok(())
    }

    fn store_stream_error(stream_error: &Arc<Mutex<Option<String>>>, message: String) {
        if let Ok(mut slot) = stream_error.lock() {
            if slot.is_none() {
                *slot = Some(message);
            }
        }
    }
}
