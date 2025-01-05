//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! コマンドラインオプション関連の処理をまとめたモジュール
//!

mod logger;

use std::sync::Arc;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

///
/// ログレベルを指し示す列挙子
///
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
#[clap(rename_all = "SCREAMING_SNAKE_CASE")]
enum LogLevel {
    /// ログを記録しない
    Off,

    /// エラー情報以上のレベルを記録
    Error,

    /// 警告情報以上のレベルを記録
    Warn,

    /// 一般情報以上のレベルを記録
    Info,

    /// デバッグ情報以上のレベルを記録
    Debug,

    /// トレース情報以上のレベルを記録
    Trace,
}

// Intoトレイトの実装
impl Into<log::LevelFilter> for LogLevel {
    fn into(self) -> log::LevelFilter {
        match self {
            Self::Off => log::LevelFilter::Off,
            Self::Error => log::LevelFilter::Error,
            Self::Warn => log::LevelFilter::Warn,
            Self::Info => log::LevelFilter::Info,
            Self::Debug => log::LevelFilter::Debug,
            Self::Trace => log::LevelFilter::Trace,
        }
    }
}

// AsRefトレイトの実装
impl AsRef<str> for LogLevel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Off => "none",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

///
/// コマンドラインオプションをまとめた構造体
///
#[derive(Parser, Debug, Clone)]
#[command(about = "Logger for environment sensor")]
#[command(version = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_COMMIT_HASH"),
    ")",
))]
#[command(long_about = None)]
pub(crate) struct Options {
    /// 記録するログレベルの指定
    #[arg(short = 'l', long = "log-level", value_name = "LEVEL",
        default_value = "INFO", ignore_case = true)]
    log_level: LogLevel,

    /// ログの出力先の指定
    #[arg(short = 'L', long = "log-output", value_name = "PATH")]
    log_output: Option<PathBuf>,

    /// 待受けを行うIPアドレス
    #[arg(short = 'b', long = "bind", default_value = "0.0.0.0")]
    bind: String,

    /// 待受けを行うTCPポート番号
    #[arg(short = 'p', long = "port", default_value = "2342")]
    port: usize,

    /// データベースファイルのパス
    #[arg(default_value = "database.db")]
    db_file: PathBuf,
}

impl Options {
    ///
    /// ログレベルへのアクセサ
    ///
    /// # 戻り値
    /// 設定されたログレベルを返す
    fn log_level(&self) -> LogLevel {
        self.log_level
    }

    ///
    /// ログの出力先へのアクセサ
    ///
    /// # 戻り値
    /// ログの出力先として設定されたパス情報を返す(未設定の場合はNone)。
    ///
    fn log_output(&self) -> Option<PathBuf> {
        self.log_output.clone()
    }

    /// 
    /// 待ち受けを行うエンドポイントへのアクセサ
    ///
    /// # 戻り値
    /// 待ち受けIPアドレス
    ///
    pub(crate) fn endpoint(&self) -> String {
        format!("{}:{}", self.bind, self.port)
    } 

    ///
    /// データベースファイルへのアクセサ
    ///
    /// # 戻り値
    /// データベースファイルファイルへのパス情報
    ///
    pub(crate) fn db_file(&self) -> PathBuf {
        self.db_file.clone()
    }

    ///
    /// 設定情報のバリデーション
    ///
    /// # 戻り値
    /// 設定情報に問題が無い場合は`Ok(())`を返す。問題があった場合はエラー情報
    /// を`Err()`でラップして返す。
    fn validate(&self) -> Result<()> {
        // ポート番号の範囲の確認
        if self.port < 1024 || self.port > 65536 {
            return Err(anyhow!("待ち受けポート番号が範囲外です。"));
        }

        Ok(())
    }
}

///
/// コマンドラインオプションのパース
///
/// # 戻り値
/// 処理に成功した場合はオプション設定をパックしたオブジェクトを`Ok()`でラップ
/// して返す。失敗した場合はエラー情報を`Err()`でラップして返す。
///
pub(super) fn parse() -> Result<Arc<Options>> {
    let opts = Options::parse();

    /*
     * 設定情報のバリデーション
     */
    opts.validate()?;

    /*
     * ログ機能の初期化
     */
    logger::init(&opts)?;

    /*
     * 設定情報の返却
     */
    Ok(Arc::new(opts))
}
