//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! プログラムのエントリーポイント
//!

mod cmd_args;
mod database;
mod receiver;
mod record;

use std::sync::Arc;

use anyhow::Result;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinHandle;

use cmd_args::Options;
use database::DatabaseTask;
use receiver::tcp::{TcpReceiveTask, TcpReceiverHandle};
use receiver::udp::{UdpReceiveTask, UdpReceiverHandle};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

///
/// 同じシグネチャを持つ複数のFutureの何れかが完了するまで待つマクロ
///
macro_rules! select_receive {
    ($($x:expr),*) => {
        tokio::select! {
            $(
                result = $x.recv() => result,
            )*
        }
    };
}

///
/// プログラムのエントリポイント
///
/// # 注記
/// main()はエラー情報の集約のみを行い、実際の処理は実行処理に記述している。
///
#[tokio::main]
async fn main() {
    /*
     * コマンドラインオプションのパース
     */
    let opts = match cmd_args::parse() {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(1);
        },
    };

    /*
     * 実行関数の呼び出し
     */
    if let Err(err) = run(opts).await {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

///
/// プログラムの実行関数
///
/// # 引数
/// * `opts` - オプション情報をパックしたオブジェクト
///
/// # 戻り値
/// プログラムが正常狩猟した場合は、`Ok(())`を返す。失敗した場合はエラー情報を
/// `Err()`でラップして返す。
///
async fn run(opts: Arc<Options>) -> Result<()> {
    info!("start env-logger {}", env!("CARGO_PKG_VERSION"));

    /*
     * 連絡用チャネルの生成 
     */
    let (tx, rx) = tokio::sync::mpsc::channel(10);

    /*
     * TCPレシーバタスクの起動
     */
    let (tcp_task, mut tcp_rx) = TcpReceiveTask::start(opts.clone()).await?;

    /*
     * UDPレシーバタスクの起動
     */
    let (udp_task, mut udp_rx) = UdpReceiveTask::start(opts.clone()).await?;

    /*
     * データベースタスクの起動
     */
    let database_task = DatabaseTask::start(opts.clone(), rx).await?;

    /*
     * シグナルトラップタスクの起動
     */
    let signal_trap_task = signal_trap(tcp_task.handle(), udp_task.handle())?;

    /*
     * 中継処理タスクの起動
     */
    let relay_task = tokio::spawn(async move {
        loop {
            match select_receive!(tcp_rx, udp_rx) {
                Some(record) => {
                    if let Err(err) = tx.send(record).await {
                        error!("record send faild: {}", err);
                    }
                }

                None => break,
            }
        }
    });

    /*
     * タスクの終了待ち
     */
    if let Err(err) = relay_task.await {
        warn!("relay task has been troubled: {}", err);
    }

    if let Err(err) = tcp_task.await {
        warn!("TCP receiver task has been troubled: {}", err);
    }

    if let Err(err) = database_task.await {
        warn!("database task has been troubled: {}", err);
    }

    if let Err(err) = signal_trap_task.await {
        warn!("signal-trap task has been troubled: {}", err);
    }

    /*
     * 終了
     */
    info!("exit env-logger process");

    Ok(())
}

///
/// シグナルトラップ処理を実行するタスク
///
/// # 引数
/// * `handle` - TCPレシーバタスクの制御を行うためのハンドルオブジェクト
///
/// # 戻り値
/// シグナルトラップタスクのジョインハンドルを返す。
///
/// # 注記
/// 本タスクでは、SIGINTと SIGTERMをトラップしする。両シグナルとも、プログラム
/// の正常終了をキックする(TCPレシーバタスクの終了を要求し、連鎖的に他のタスク
/// を終了させる)。
///
fn signal_trap(tcp_handle: TcpReceiverHandle, udp_handle: UdpReceiverHandle)
    -> Result<JoinHandle<()>>
{
    /*
     * シグナルレシーバオブジェクトを生成
     */
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    /*
     * タスクを起動
     */
    Ok(tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => info!("caught SIGINT"),
            _ = sigterm.recv() => info!("caught SIGTERM"),
        }

        tcp_handle.shutdown().await;
        udp_handle.shutdown().await;
    }))
}
