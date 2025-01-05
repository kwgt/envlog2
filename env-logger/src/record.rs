//
// Logger for environment sensors
//
//  Copyright 2025 (C) Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! レコード定義を行うモジュール
//!

use anyhow::{anyhow, Result};
use chrono::{Local, TimeZone, Utc};
use serde::Deserialize;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

///
/// センサーから受信したデータのレコードを投影する構造体
///
#[derive(Debug, Deserialize)]
pub(crate) struct SensorRecord {
    /// 送信デバイスの設置場所
    location: String,

    /// 送信デバイス固有のID
    device_id: Option<String>,

    /// タイムスタンプ
    #[serde(skip)]
    timestamp: u64, 

    /// 気温
    temperature: Option<f32>,

    /// 湿度
    humidity: Option<f32>,

    /// 気圧
    air_pressure: Option<f32>,
}

impl SensorRecord {
    ///
    /// JSONからの変換関数
    ///
    /// # 引数
    /// * `json` - デバイスから受け取ったJSON文字列
    ///
    /// # 戻り値
    /// JSONから変換したセンサーデータ
    ///
    /// # 注記
    /// タイムスタンプは本関数で取得する (JSONには該当するプロパティは存在しな
    /// い)。
    ///
    pub(crate) fn from_json<'a>(json: &'a str) -> Result<Self> {
        match serde_json::from_str::<SensorRecord>(json) {
            Ok(mut value) => {
                value.timestamp = Utc::now().timestamp_millis() as u64;
                Ok(value)
            }

            Err(err) => Err(anyhow!("{}", err)),
        }
    }

    ///
    /// デバイス設置場所へのアクセサ
    ///
    /// # 戻り値
    /// 設置場所を文字列で返す
    ///
    pub(crate) fn location(&self) -> String {
        self.location.clone()
    }

    ///
    /// デバイス固有のIDへのアクセサ
    ///
    /// # 戻り値
    /// デバイス固有IDが設定されている場合は、IDを`Some()`でラップして返す。
    ///
    pub(crate) fn device_id(&self) -> Option<String> {
        self.device_id.clone()
    }

    ///
    /// タイムスタンプへのアクセサ
    ///
    /// # 戻り値
    /// タイムスタンプをミリ秒単位のUNIX時刻で返す
    ///
    pub(crate) fn timestamp(&self) -> u64 {
        self.timestamp
    }

    ///
    /// 気温データへのアクセサ
    ///
    /// # 戻り値
    /// 気温データが取得できている場合は値を`Some()`でラップして返す
    ///
    pub(crate) fn temperature(&self) -> Option<f32> {
        self.temperature.clone()
    }

    ///
    /// 湿度データへのアクセサ
    ///
    /// # 戻り値
    /// 湿度データが取得できている場合は値を`Some()`でラップして返す
    ///
    pub(crate) fn humidity(&self) -> Option<f32> {
        self.humidity.clone()
    }

    ///
    /// 気圧データへのアクセサ
    ///
    /// # 戻り値
    /// 気圧データが取得できている場合は値を`Some()`でラップして返す
    ///
    pub(crate) fn air_pressure(&self) -> Option<f32> {
        self.air_pressure.clone()
    }
}

// ToStringトレイトの実装
impl ToString for SensorRecord {
    fn to_string(&self) -> String {
        let mut vals = vec![];

        if let Some(val) = &self.device_id {
            vals.push(val.clone());
        }

        if let Some(val) = self.temperature {
            vals.push(format!("{:.1}\u{00b0}\u{0043}", val));
        }

        if let Some(val) = self.humidity {
            vals.push(format!("{:.1}%", val));
        }

        if let Some(val) = self.air_pressure {
            vals.push(format!("{:.1}hpa", val));
        }

        format!(
            "\"{}\",{},{}",
            self.location,
            local_time_string(self.timestamp),
            vals.join(",")
        )
    }
}

///
/// ミリ秒単位のUNIX時刻をローカルタイム表現の文字列に変換する
///
/// # 引数
/// * `tm` - 変換対象のミリ秒単位のUNIX時刻
///
/// # 戻り値
/// ローカルタイムでの表記に変換した文字列
///
fn local_time_string(tm: u64) -> String {
    Utc.timestamp_opt((tm / 1000) as i64, ((tm % 1000) * 1000000) as u32)
        .unwrap()
        .with_timezone(&Local)
        //.format("%Y/%m/%d %H:%M:%S").to_string()
        .to_string()
}
