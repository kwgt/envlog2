create table if not exists SENSOR_RESULT_TABLE (
  /* デバイスの設置場所名 */
  location TEXT not NULL,

  /* デバイス固有のID */
  device_id TEXT,

  /* 登録時刻(ミリ秒単位のUNIX時刻) */
  timestamp INTEGER not NULL,

  /* 気温(摂氏) */
  temperature REAL,

  /* 湿度(相対) */
  humidity REAL,

  /* 気圧(hpa) */
  air_pressure REAL,

  /* プライマリーキー設定 */
  primary key(location, timestamp)
);
