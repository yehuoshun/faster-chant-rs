use anyhow::Result;
use log::LevelFilter;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 日志管理器
pub struct Logger;

impl Logger {
    /// 初始化日志系统
    /// `debug_mode`: true 时输出 DEBUG 级别，否则 INFO
    /// `log_dir`: 日志文件目录（exe 同目录）
    pub fn init(debug_mode: bool, log_dir: PathBuf) -> Result<()> {
        fs::create_dir_all(&log_dir)?;

        let level = if debug_mode {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };

        // 获取今天的日志文件名
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let log_file = log_dir.join(format!("faster-chant-{}.log", today));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        // 组合 logger：同时输出到控制台和文件
        let file_logger = FileLogger::new(file);
        let console_logger = ConsoleLogger;

        let combined = CombinedLogger {
            loggers: vec![Box::new(console_logger), Box::new(file_logger)],
        };

        log::set_boxed_logger(Box::new(combined))?;
        log::set_max_level(level);

        // 清理 7 天前的旧日志
        Self::cleanup_old_logs(&log_dir, 7);

        log::info!("日志系统初始化完成，级别: {:?}", level);
        log::info!("日志文件: {}", log_file.display());
        Ok(())
    }

    fn cleanup_old_logs(dir: &PathBuf, keep_days: u32) {
        if let Ok(entries) = fs::read_dir(dir) {
            let cutoff = chrono::Local::now()
                .date_naive()
                .checked_sub_days(chrono::Days::new(keep_days as u64));
            if let Some(cutoff) = cutoff {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("faster-chant-") && name.ends_with(".log") {
                        let date_str = &name[13..23]; // "faster-chant-".len() = 13
                        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            if date < cutoff {
                                let _ = fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 组合日志器
struct CombinedLogger {
    loggers: Vec<Box<dyn log::Log>>,
}

impl log::Log for CombinedLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.loggers.iter().any(|l| l.enabled(metadata))
    }

    fn log(&self, record: &log::Record) {
        for logger in &self.loggers {
            if logger.enabled(record.metadata()) {
                logger.log(record);
            }
        }
    }

    fn flush(&self) {
        for logger in &self.loggers {
            logger.flush();
        }
    }
}

/// 控制台日志器
struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= LevelFilter::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let ts = chrono::Local::now().format("%H:%M:%S%.3f");
            let level = match record.level() {
                log::Level::Error => "ERROR",
                log::Level::Warn => "WARN ",
                log::Level::Info => "INFO ",
                log::Level::Debug => "DEBUG",
                log::Level::Trace => "TRACE",
            };
            println!(
                "[{}] [{}] [{}] {}",
                ts,
                level,
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// 文件日志器
struct FileLogger {
    file: Mutex<File>,
}

impl FileLogger {
    fn new(file: File) -> Self {
        Self {
            file: Mutex::new(file),
        }
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= LevelFilter::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level = match record.level() {
                log::Level::Error => "ERROR",
                log::Level::Warn => "WARN ",
                log::Level::Info => "INFO ",
                log::Level::Debug => "DEBUG",
                log::Level::Trace => "TRACE",
            };
            let line = format!(
                "[{}] [{}] [{}] {}\n",
                ts,
                level,
                record.target(),
                record.args()
            );
            if let Ok(mut file) = self.file.lock() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush(); // 立即写入，防止崩溃丢日志
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

// ── 日志宏辅助 ──

/// 记录 OCR 结果（含区域信息）
#[macro_export]
macro_rules! log_ocr {
    ($region:expr, $text:expr) => {
        log::debug!(
            "OCR [{}] x={:.2} y={:.2} w={:.2} h={:.2} → {:?}",
            stringify!($region),
            $region.x,
            $region.y,
            $region.w,
            $region.h,
            $text
        )
    };
}

/// 记录状态转换
#[macro_export]
macro_rules! log_transition {
    ($from:expr, $to:expr) => {
        log::info!("状态转换: {:?} → {:?}", $from, $to)
    };
}

/// 记录 KDA 变化
#[macro_export]
macro_rules! log_kda {
    ($prev:expr, $curr:expr, $event:expr) => {
        log::info!(
            "KDA {}: {}/{}/{} → {}/{}/{} (事件: {:?})",
            stringify!($event),
            $prev.kills, $prev.deaths, $prev.assists,
            $curr.kills, $curr.deaths, $curr.assists,
            $event
        )
    };
}

/// 记录发送消息
#[macro_export]
macro_rules! log_send {
    ($msg:expr, $channel:expr) => {
        log::info!(
            "发送[{}] ({})字: {}",
            $channel,
            $msg.chars().count(),
            if $msg.chars().count() > 30 {
                format!("{}...", &$msg[..30])
            } else {
                $msg.to_string()
            }
        )
    };
}

/// 记录错误并附加上下文
#[macro_export]
macro_rules! log_error_ctx {
    ($ctx:expr, $err:expr) => {
        log::error!("{}: {} (来源: {}:{}:{})", $ctx, $err, file!(), line!(), column!())
    };
}