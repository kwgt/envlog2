//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! データベース処理をまとめたモジュール
//!

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use rusqlite::{named_params, Connection};
use tokio::task::JoinHandle;
use tokio::sync::mpsc::Receiver;

use crate::cmd_args::Options;
use crate::record::SensorRecord;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

/// テーブル作成のクエリー
const CREATE_TABLE_QUERY: &str = include_str!("../data/create_table.sql");

/// データベース最適化クエリー
const VACUUM_QUERY: &str = include_str!("../data/vacuum.sql");

/// レコード挿入クエリー
const INSERT_RECORD_QUERY: &str = include_str!("../data/insert_record.sql");

///
/// データベース処理タスクをラップする構造体
///
pub(crate) struct DatabaseTask {
    /// タスクのジョインハンドル
    handle: JoinHandle<()>,
}

impl DatabaseTask {
    ///
    /// タスクの開始
    ///
    /// # 引数
    /// * `opts` - オプション情報をパックしたオブジェクト
    /// * `rx` - 受信レコード受信用チャネルオブジェクト
    ///
    /// # 戻り値
    /// タスクの開始に成功した場合は、タスクにバインドされたDatabaseTaskのオブ
    /// ジェクト(Futureトレイトを実装)を`Ok()`でラップして返す。
    /// 失敗した場合はエラー情報を `Err()`でラップして返す。
    pub(crate) async fn start(
        opts: Arc<Options>,
        pipeline_rx: Receiver<SensorRecord>
    ) -> Result<Self>
    {
        /*
         * データベースのオープン
         */
        let conn = open_database(opts.db_file())?;

        info!("success open {}", opts.db_file().display());

        /*
         * データベースタスクの起動
         */
        let handle = tokio::spawn(database_task(conn, pipeline_rx));

        /*
         * 戻り値の生成
         */
        Ok(Self {handle})
    }
}

// Futureトレイトの実装
impl Future for DatabaseTask {
    type Output = std::result::Result<(), tokio::task::JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().handle).poll(cx)
    }
}

///
/// データベースオープン手続きをまとめた関数
///
/// # 引数
/// * 'path' - データベースファイルへのパス
///
/// # 戻り値
/// データベースのオープンに成功した場合は、接続オブジェクトを`Ok()`でラップし
/// て返す。
///
fn open_database(path: impl AsRef<Path>) -> Result<Connection> {
    /*
     * データベースのオープン
     */
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(err) => return Err(anyhow!("databse open failed: {}", err)),
    };

    /*
     * テーブルの生成
     */
    if let Err(err) = conn.execute(CREATE_TABLE_QUERY, []) {
        return Err(anyhow!("create table failed: {}", err))
    }

    /*
     * データベースの最適化
     */
    if let Err(err) = conn.execute(VACUUM_QUERY, []) {
        return Err(anyhow!("vacuum failed: {}", err))
    }

    /*
     * 戻り値の返却
     */
    Ok(conn)
}

///
/// データベース処理タスク
///
/// # 引数
/// * `conn` - データベース接続オブジェクト
/// * `pipeline_rx` - 受信レコード受信チャネルオブジェクト
///
async fn database_task(
    conn: Connection,
    mut pipeline_rx: Receiver<SensorRecord>
)
{
    info!("start database task");

    while let Some(record) = pipeline_rx.recv().await {
        if let Err(err) = insert_record(&conn, &record) {
            error!("insert record failed: {}", err);
            continue;
        } 

        info!("insert record: {}", record.to_string());
    }

    info!("shutdown database task");
}

///
/// レコードのインサート手続きをまとめた関数
///
/// # 引数
/// * `conn` - データベース接続オブジェクト
/// * `record` -  受信レコード
///
/// # 戻り値
/// レコードのインサートに成功した場合は`Ok(())`を返す。失敗した場合はエラー情
/// 報を`Err()`でラップして返す。
///
fn insert_record(conn: &Connection, record: &SensorRecord)
    -> rusqlite::Result<()>
{
    conn.execute(
        INSERT_RECORD_QUERY,
        named_params! {
            ":location" : record.location(),
            ":device_id" : record.device_id(),
            ":timestamp" : record.timestamp(),
            ":temperature" : record.temperature(),
            ":humidity" : record.humidity(),
            ":air_pressure" : record.air_pressure(),
        },
    )?;

    Ok(())
}
