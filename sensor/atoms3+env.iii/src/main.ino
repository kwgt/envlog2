/*
 * sensor application for AtomS3 + ENV.III
 *
 *  Copyright (C) 2025 Hiroshi Kuwagata <kgt9221@gmail.com>
 */

#undef USE_UDP

#include <M5AtomS3.h>
#include <M5Unified.h>

#include <freertos/FreeRTOS.h>
#include <freertos/timers.h>

#include <FastLED.h>
#include <Wire.h>
#include <WiFi.h>
#ifdef USE_UDP
#include <WiFiUdp.h>
#endif /* defined(USE_UDP) */
#include <M5UnitENV.h>
#include <ArduinoJson.h>
#include <math.h>

//! 設置場所の名前
#define LOCATION_NAME   ("2F寝室")

//! アクセスポイントへの接続情報(SSID)
#define AP_SSID         ("aterm-78e0c4-g")

//! アクセスポイントへの接続情報(パスワード)
#define AP_PASSWORD     ("08c3fd4387187")

//! サーバの待ち受けアドレス
#define SERVER_ADDR     ("192.168.10.119")
//#define SERVER_ADDR     ("192.168.10.110")

//! サーバの待ち受けポート番号
#define SERVER_PORT     (2342)

//! コネクションタイムアウト(ミリ秒)
#define CONNECT_TIMEOUT (10000)

//! アクセスポイント接続のリトライ回数
#define RETRY_LIMIT     (10)

//! RGBLED制御に割り当てられているGPIOの番号
#define LED_PIN         (35)

//! センサー地の取得間隔(秒)
#define SENSOR_INTERVAL (120)

//! FastLEDで使用するフレームバッファ
CRGB led;

//! 温湿度センサーへのアクセスインタフェース
SHT3X sht;

//! 気圧センサーへのアクセスインタフェース
QMP6988 qmp;

#ifdef USE_UDP
WiFiUDP udp;
#else /* defined(USE_UDP) */
//! TCPクライアントオブジェクト
WiFiClient tcp;
#endif /* defined(USE_UDP) */

/**
 * 設定関数
 */
void
setup()
{
  bool err;

  /*
   * ボードの初期化
   */
  M5.begin();

  /*
   * FastLEDの初期化
   */
  FastLED.addLeds<WS2812, LED_PIN, GRB>(&led, 1); 
  FastLED.setBrightness(20);

  /*
   * I2Cの初期化
   */
  Wire.begin(M5.Ex_I2C.getSDA(), M5.Ex_I2C.getSCL(), 100000);

  /*
   * シリアルの初期化
   */
  Serial.begin(115200);

  /*
   * センサーインタフェースの初期化
   */
  err = qmp.begin(
    &Wire,
    QMP6988_SLAVE_ADDRESS_L,
    M5.Ex_I2C.getSDA(),
    M5.Ex_I2C.getSCL(),
    400000U
  );

  if (!err) {
    abort_system("QMP6988 initialize failed");
  }

  err = sht.begin(
    &Wire,
    SHT3X_I2C_ADDR,
    M5.Ex_I2C.getSDA(),
    M5.Ex_I2C.getSCL(),
    400000U
  );

  if (!err) {
    abort_system("SHT30 initialize failed");
  }

  /*
   * タイマーの起動
   */
  TimerHandle_t timer = xTimerCreate(
    "SensorTimer",
    pdMS_TO_TICKS(SENSOR_INTERVAL * 1000),
    pdTRUE,
    (void*)xTaskGetCurrentTaskHandle(),
    timer_callback
  );

  xTimerStart(timer, 0);
}

/**
 * ルーパー関数
 */
void
loop()
{
  /*
   * タイマーからの通知を待つ
   */
  ulTaskNotifyTake(pdTRUE, portMAX_DELAY);

  /*
   * WiFiアクセスポイントへの接続
   */
  if (connect(AP_SSID, AP_PASSWORD)) {
    JsonDocument doc;
    static char mac[18];
    static char json[192];
    size_t len;

    /*
     * センサーからのデータの受信
     */
    sht.update();
    qmp.update();

    /*
     * MACの取得
     */
    get_mac_address(mac);

    /*
     * JSONへのシリアライズ
     */
    doc["location"] = LOCATION_NAME;
    doc["device_id"] = mac;
    doc["temperature"] = sht.cTemp;
    doc["humidity"] = sht.humidity;
    doc["air_pressure"] = qmp.pressure / 100.0;

    len = serializeJson(doc, json);
    Serial.println(json);

    /*
     * サーバへの送信
     */
#ifdef USE_UDP
    udp.beginPacket(SERVER_ADDR, SERVER_PORT);
    udp.write((uint8_t*)json, len);
    udp.flush();
    udp.endPacket();

    delay(1000);

#else /* defined(USE_UDP) */
    if (tcp.connect(SERVER_ADDR, SERVER_PORT, CONNECT_TIMEOUT)) {
      tcp.println(json);
      tcp.flush();

      while (tcp.connected()) {
        delay(50);
      }

      tcp.stop();

    } else {
      emit_led(CRGB::Magenta);
    }
#endif /* defined(USE_UDP) */

    disconnect();
  }
}

/**
 * タイマーのコールバック関数
 * 
 * @param [in] timer  登録元のタイマーハンドラ
 */
static void
timer_callback(TimerHandle_t timer)
{
  TaskHandle_t task = (TaskHandle_t)pvTimerGetTimerID(timer);

  if (task != NULL) {
    /*
     * 通知を発行
     */
    xTaskNotifyGive(task);
  }
}

/**
 * WiFiアクセスポイントへの接続
 *
 * @param [in] ssid  アクセスポイントのSSID
 * @param [in] passwd  アクセスポイントのパスワード
 */
static bool
connect(const char* ssid, const char* passwd)
{
  bool ret;
  int i;

  emit_led(CRGB::Yellow);

  WiFi.begin(AP_SSID, AP_PASSWORD);

  for (i = 0; i < RETRY_LIMIT; i++) {
    delay(2000);
    if (WiFi.status() == WL_CONNECTED) break;
  }

  if (i < RETRY_LIMIT) {
      emit_led(CRGB::Green);
      ret = true;
  } else {
      emit_led(CRGB::Red);
      ret = false;
  }

  return ret;
}

/**
 * WiFiアクセスポイントからの切断
 */
static void
disconnect()
{
  WiFi.disconnect(true, true);
  extinguish_led();
}

/**
 * ESP32のデフォルトMACアドレスの取得
 *
 * @param [out] dst  取得したMACアドレスの書き込み先
 *
 * @desc
 *  マイコンから読み出したMACアドレスを文字列化して引数dstで指定した領域に書き
 *  込みを行う。
 */
static void
get_mac_address(char dst[18])
{
  uint8_t mac[6] = {0};

  esp_efuse_mac_get_default(mac);
  sprintf(
    dst,
    "%02x:%02x:%02x:%02x:%02x:%02x",
    mac[0],
    mac[1],
    mac[2],
    mac[3],
    mac[4],
    mac[5]
  );
}

/**
 * LEDの点灯
 *
 * @param [in] color  LEDの点灯色
 */
static void
emit_led(CRGB color)
{
  led = color;
  FastLED.show();
}

/**
 * LEDの消灯
 */
static void
extinguish_led()
{
  FastLED.clear(true);
}

/**
 * システムのアボート
 */
static void
abort_system(const char* reason)
{
  Serial.print("system aborted: ");
  Serial.println(reason);

  vTaskSuspendAll();
  vTaskDelete(NULL);
}
