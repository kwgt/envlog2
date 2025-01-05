//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! UDP受信処理をまとめたモジュール
//!

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use rhexdump::rhexdumps;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

use crate::record::SensorRecord;
use crate::cmd_args::Options;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
///
/// 受信処理タスクをラップする構造体
///
pub(crate) struct UdpReceiveTask {
    /// タスクのジョインハンドル
    handle: JoinHandle<()>,

    /// タスクへのリクエスト通知用のチャネル
    request_tx: Sender<TaskRequest>,
}

impl UdpReceiveTask {
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
        let sock = match UdpSocket::bind(opts.endpoint()).await {
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
    pub(crate) fn handle(&self) -> UdpReceiverHandle {
        UdpReceiverHandle {request_tx: self.request_tx.clone()}
    }
}

// Futureトレイトの実装
impl Future for UdpReceiveTask {
    type Output = std::result::Result<(), tokio::task::JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().handle).poll(cx)
    }
}

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
/// レシーバタスク制御用のハンドル構造体
///
pub(crate) struct UdpReceiverHandle {
    /// シャットダウン要求送信用オブジェクト
    request_tx: Sender<TaskRequest>,
}

impl UdpReceiverHandle {
    ///
    /// タスクの終了要求の発行
    ///
    pub(crate) async fn shutdown(&self) {
        let _ = self.request_tx.send(TaskRequest::Shutodwn).await;
    }
}

///
/// UDPリスナー処理を行うタスク
///
/// # 引数
/// * `sock` - TCPポートにバインドされたリスナーソケットオブジェクト
/// * `pipeline_tx` - 受信レコード送信用チャネルオブジェクト
/// * `shutdown_rx` - シャットダウン要求受信用チャネルオブジェクト
///
async fn listener_task(
    sock: UdpSocket,
    pipeline_tx: Sender<SensorRecord>,
    mut request_rx: Receiver<TaskRequest>,
) {
    info!("start UDP receiver task");

    // レコード送信チャネルを複製できるようにArcでラップ
    let pipeline_tx = Arc::new(pipeline_tx);

    loop {
        let mut buff = vec![0; 1024];

        tokio::select! {
            // バインドポートへの接続があった場合
            result = sock.recv_from(&mut buff) => {
                match result {
                    Ok((len, addr)) => {
                        info!("receive from: {:?}", addr);

                        tokio::spawn(receive_task(
                            buff[..len].to_vec(),
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

    info!("shutdown UDP receiver task");
}

///
/// データ受信処理を行うタスク
///
/// # 引数
/// * `json` - TCPセッションタスク
/// * `pipeline_tx` - 受信レコード送信用チャネルオブジェクト
///
async fn receive_task(
    data: Vec<u8>,
    pipeline_tx: Arc<Sender<SensorRecord>>
) 
{
    debug!("received data:\n{}", rhexdumps!(&data));

    match String::from_utf8(data) {
        Ok(json) => {
            let record = match SensorRecord::from_json(&json) {
                Ok(record) => record,
                Err(err) => {
                    error!("invalid JSON received: {}", err);
                    return;
                }
            };

            if let Err(err) = pipeline_tx.send(record).await {
                error!("send sensor result failed: {}", err);
            }
        }

        Err(err) => error!("invalid JSON received: {}", err),
    }
}
