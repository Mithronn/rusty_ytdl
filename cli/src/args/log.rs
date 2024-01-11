use std::fmt::Arguments;

use clap::Parser;
use fern::colors::{Color, ColoredLevelConfig};
use fern::FormatCallback;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{LevelFilter, Record};

#[derive(Parser)]
pub struct LogArgs {
    /// Sets the log-level of rusty_ytdl [default: Info]
    /// (-v = Error, ..., -vvvvv = Trace)
    /// (other crates have log level Warn)
    #[clap(
        long, 
        short,
        action = clap::ArgAction::Count,
        global = true,
    )]
    verbose: u8,

    /// Show a progress bar
    #[clap(long, conflicts_with = "verbose")]
    progress: bool,

    /// Turn off logging for all crates
    #[clap(long, short, conflicts_with = "verbose")]
    quiet: bool,
}

impl LogArgs {
    pub fn init_logger(&self) {
        if self.quiet || self.progress {
            return;
        }

        let formatter = self.log_msg_formatter();

        fern::Dispatch::new()
            .level(log::LevelFilter::Warn)
            .level_for("rusty_ytdl", self.level_filter())
            .format(formatter)
            .chain(std::io::stdout())
            .apply()
            .expect("The global logger was already initialized");
    }

    pub fn init_progress_bar(&self, total: u64) -> ProgressBar {
        let pb = ProgressBar::new(total);

        pb.set_style(
            ProgressStyle::with_template("{msg}\n\n{spinner:.blue} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                .progress_chars("█░░")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        );

        pb
    }

    fn log_msg_formatter(&self) -> fn(FormatCallback, &Arguments, &Record) {
        #[inline(always)]
        fn format_msg(
            out: FormatCallback,
            level: impl std::fmt::Display,
            record: &Record,
            msg: &Arguments,
        ) {
            out.finish(format_args!(
                "{:<5} [{}:{}]: {}",
                level,
                record.target(),
                record.line().unwrap_or_default(),
                msg,
            ))
        }

        // match self.color {
        //     ColorUsage::Always => |out: FormatCallback, message: &Arguments, record: &Record| {
        //         static COLORS: ColoredLevelConfig = ColoredLevelConfig {
        //             error: Color::Red,
        //             warn: Color::Yellow,
        //             info: Color::Green,
        //             debug: Color::BrightBlue,
        //             trace: Color::White,
        //         };

        //         format_msg(out, COLORS.color(record.level()), record, message);
        //     },
        //     ColorUsage::Never => |out: FormatCallback, message: &Arguments, record: &Record| {
        //         format_msg(out, record.level(), record, message);
        //     },
        // }

        |out: FormatCallback, message: &Arguments, record: &Record| {
            static COLORS: ColoredLevelConfig = ColoredLevelConfig {
                error: Color::Red,
                warn: Color::Yellow,
                info: Color::Green,
                debug: Color::BrightBlue,
                trace: Color::White,
            };

            format_msg(out, COLORS.color(record.level()), record, message);
        }
    }

    fn level_filter(&self) -> log::LevelFilter {
        match self.verbose {
            1 => LevelFilter::Error,
            2 => LevelFilter::Warn,
            0 | 3 => LevelFilter::Info,
            4 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

#[derive(Parser)]
enum ColorUsage {
    Always,
    Never,
}
