//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! TCP受信処理をまとめたモジュール
//!

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use rhexdump::rhexdumps;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};

use crate::record::SensorRecord;
use crate::cmd_args::Options;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

/// データ受信タイムアウト(秒)
const DATA_TIMEOUT: u64 = 10;

///
/// タスクに対するリクエスト
///
/// # 注記
/// 現時点ではシャットダウンしかないが、将来の拡張用にenumで定義しておく。
///
enum TaskRequest {
    /// シャットダウン要求
    Shutodwn,
}

///
/// 受信処理タスクをラップする構造体
///
pub(crate) struct TcpReceiveTask {
    /// タスクのジョインハンドル
    handle: JoinHandle<()>,

    /// タスクへのリクエスト通知用のチャネル
    request_tx: Sender<TaskRequest>,
}

impl TcpReceiveTask {
    ///
    /// タスクの開始
    ///
    /// # 引数
    /// * `opts` - オプション情報をまとめたオブジェクト
    ///
    /// # 戻り値
    /// タスクの開始に成功した場合は、タスクにバインドされたTcpReceiveTaskのオ
    /// ブジェクト(Futureトレイトを実装)と、受信レコードの受信用のチャネルオブ
    /// ジェクトをパックしたタプルを`Ok()`でラップして返す。
    /// 失敗した場合はエラー情報を `Err()`でラップして返す。
    ///
    pub(crate) async fn start(opts: Arc<Options>)
        -> Result<(Self, Receiver<SensorRecord>)>
    {
        /*
         * リスナーオブジェクトの生成(TCPポートのバインド)
         */
        let sock = match TcpListener::bind(opts.endpoint()).await {
            Ok(sock) => sock,
            Err(err) => return Err(anyhow!("bind failed: {}", err)),
        };

        info!("success bind to {}", opts.endpoint());

        /*
         * チャネルオブジェクトの生成
         */
        let (pipeline_tx, pipeline_rx) = tokio::sync::mpsc::channel(10);
        let (request_tx, request_rx) = tokio::sync::mpsc::channel(5);

        /*
         * リスナータスクの起動
         */
        let handle =tokio::spawn(listener_task(
            sock,
            pipeline_tx,
            request_rx,
        ));

        /*
         * 戻り値の生成
         */
        Ok((Self {handle, request_tx}, pipeline_rx))
    }

    ///
    /// 制御用ハンドルの取得
    ///
    /// # 戻り値
    /// 制御用ハンドルオブジェクトを返す。
    ///
    pub(crate) fn handle(&self) -> TcpReceiverHandle {
        TcpReceiverHandle {request_tx: self.request_tx.clone()}
    }
}

// Futureトレイトの実装
impl Future for TcpReceiveTask {
    type Output = std::result::Result<(), tokio::task::JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().handle).poll(cx)
    }
}

///
/// レシーバタスク制御用のハンドル構造体
///
pub(crate) struct TcpReceiverHandle {
    /// シャットダウン要求送信用オブジェクト
    request_tx: Sender<TaskRequest>,
}

impl TcpReceiverHandle {
    ///
    /// タスクの終了要求の発行
    ///
    pub(crate) async fn shutdown(&self) {
        let _ = self.request_tx.send(TaskRequest::Shutodwn).await;
    }
}

///
/// TCPリスナー処理を行うタスク
///
/// # 引数
/// * `sock` - TCPポートにバインドされたリスナーソケットオブジェクト
/// * `pipeline_tx` - 受信レコード送信用チャネルオブジェクト
/// * `shutdown_rx` - シャットダウン要求受信用チャネルオブジェクト
///
async fn listener_task(
    sock: TcpListener,
    pipeline_tx: Sender<SensorRecord>,
    mut request_rx: Receiver<TaskRequest>,
)
{
    info!("start TCP receiver task");

    // レコード送信チャネルを複製できるようにArcでラップ
    let pipeline_tx = Arc::new(pipeline_tx);

    loop {
        tokio::select! {
            // バインドポートへの接続があった場合
            result = sock.accept() => {
                match result {
                    Ok((sock, addr)) => {
                        info!("connection from: {:?}", addr);

                        tokio::spawn(session_task(
                            sock,
                            pipeline_tx.clone()
                        ));
                    }

                    Err(err) => error!("accept failed: {}", err),
                }
            }

            // 制御チャネルにリクエストが届いた場合
            request = request_rx.recv() => {
                match request {
                    Some(TaskRequest::Shutodwn) => break,
                    None => { /* ignore */ }
                }
            }
        }
    }

    info!("shutdown TCP receiver task");
}

///
/// セッション処理を行うタスク
///
/// # 引数
/// * `sock` - TCPセッションタスク
/// * `pipeline_tx` - 受信レコード送信用チャネルオブジェクト
///
async fn session_task(
    sock: TcpStream,
    pipeline_tx: Arc<Sender<SensorRecord>>
) 
{
    /*
     * クライアントからのデータを受信
     */
    let duration = Duration::from_secs(DATA_TIMEOUT);
    let record = match timeout(duration, receive_record(sock)).await {
        Ok(Ok(record)) => record,

        Ok(Err(err)) => {
            error!("{}", err);
            return;
        }

        Err(err) => {
            error!("data receive timeout: {}", err);
            return;
        }
    };

    /*
     * 受信したレコードの送信
     */
    if let Err(err) = pipeline_tx.send(record).await {
        error!("send sensor result failed: {}", err);
    }
}

///
/// JSONの受信
///
/// # 引数
/// * `sock` - ソケットオブジェクト
///
/// # 戻り値
/// 受信に成功した場合は受信したJSONを`Ok()`でラップして返す。
///
/// # 概要、
///
async fn receive_record(mut sock: TcpStream) -> Result<SensorRecord> {
    let reader = BufReader::new(&mut sock);

    /*
     * 1行分のデータを受信
     */
    let ret = match reader.lines().next_line().await {
        Ok(result) => {
            if let Some(json) = result {
                /*
                 * データが受信できたらJSONとしてパース
                 */
                debug!("received data:\n{}", rhexdumps!(&json));

                match SensorRecord::from_json(&json) {
                    Ok(record) => Ok(record),
                    Err(err) => Err(anyhow!("parse JSON failed: {}", err)),
                }

            } else {
                /*
                 * データが受信できていなかったらエラー
                 */
                Err(anyhow!("receive data is empty"))
            }
        } 

        Err(err) => Err(anyhow!("TCP receive failed: {}", err)),
    };

    /*
     * 後始末としてセッションを切断
     */
    if let Err(err) = sock.shutdown().await {
        return Err(anyhow!("TCP socket shutdown failed: {}", err));
    }

    ret
}
