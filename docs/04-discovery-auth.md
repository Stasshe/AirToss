# 04. 発見と認証（BLE）

BLE の役割は、発見、能力交換、鍵合意、経路情報の受け渡しに限る。
ファイル本体は送らない（低速 fallback を利用者が明示的に選んだ場合のみ、`05-transport.md` の BleGattTransport を使う）。

## 設計の前提となる失敗事例

Flying Carpet は、暗号化必須属性を持つ GATT characteristic と OS ペアリングの順序に起因して、GATT service を解決できない組み合わせを抱えている。
また、Bluetooth アダプターへの操作が既存周辺機器の切断を引き起こした（実機確認）。
そこで AirToss は次の 2 点を設計の柱にする。

1. **OS の Bluetooth ペアリングに依存しない**。GATT characteristic に暗号化必須属性を付けず、機密性はアプリケーション層の鍵交換で確保する。
2. **アダプター状態に触れない**。scan と advertise と GATT 接続だけを行い、それ以外の Bluetooth 操作を持たない。

## 役割

両端末が同時に **Advertiser**（GATT Server）かつ **Scanner**（GATT Client）として動く。
これにより「送る側」「受ける側」の事前の役割分担が不要になり、ホーム画面で双方に相手が見える。

GATT 接続は、利用者が相手をタップした側が Client となって開始する。
双方が同時にタップした場合は、セッション ID の辞書順比較で片方の接続だけを残す。

## Advertising

Service UUID（固定値）：

```
A1270550-41B2-4055-9E10-A1270550C0DE
```

Advertising payload（Service Data、31 バイト制限内に収める）：

```
protocol_version   u8
session_device_id  8 bytes   セッションごとに乱数生成。恒久 ID を使わない
capability_flags   u16       bit0: wifi_aware
                             bit1: wifi_direct_concurrent
                             bit2: ap_host
                             bit3: lan_connected
platform           u8        0:iOS 1:Android 2:Windows 3:macOS 4:Linux
```

表示名は payload に入れず、GATT の `info` characteristic から読む（サイズ制限とプライバシーのため）。

## GATT レイアウト

Service 配下に characteristic を 2 つだけ置く。
いずれも暗号化必須属性、認証必須属性を付けない。

| Characteristic | UUID 末尾 | 属性 | 用途 |
|---|---|---|---|
| `info` | `...0001` | Read | 表示名、platform、詳細 capability の CBOR |
| `channel` | `...0002` | Write Without Response, Notify | 交渉メッセージの双方向ストリーム |

`channel` 上のメッセージは、`u16` 長さプレフィックス付き CBOR とし、MTU を超えるものは断片化して再構成する（ATT MTU は 23 バイトまで落ちる前提で実装する）。

## ハンドシェイク

Client を A、Server を B とする。

```
A → B : Hello { version, session_device_id_A, eph_pub_A }   eph_pub は X25519 一時公開鍵
B → A : Hello { version, session_device_id_B, eph_pub_B }

両者 : shared = X25519(eph_priv, eph_pub_peer)
両者 : transcript = SHA-256(Hello_A || Hello_B)
両者 : keys = HKDF-SHA256(ikm = shared, salt = transcript,
                          info = "airtoss v1")
        → k_auth, k_code

両者 : code = SAS(k_code)      6 桁の短認証文字列として画面に表示

A → B : Confirm { mac = HMAC(k_auth, "confirm-a" || transcript) }
B → A : Confirm { mac = HMAC(k_auth, "confirm-b" || transcript) }
```

6 桁コードの一致確認は人間が行う。
コードは transcript から導出されるため、中間者が介在すると両画面のコードが一致しない。
双方が UI で「一致している」を押し、かつ Confirm の MAC 検証が通った時点で、セッションを **認証済み** とする。

`k_auth` は、フレーム暗号化には使わない。
セッション中に確立するすべての接続（初回接続、および途中で切れた場合の再接続すべて）を認証するための長期鍵として、セッション終了まで保持する。
接続ごとの実際の暗号鍵は、接続確立の都度 `06-transfer-protocol.md` の Rekey ハンドシェイクで新規に導出する。
これにより、複数の物理接続にまたがって同一の暗号鍵を使い回すことがなくなり、nonce 再利用の危険が構造的に生じない。
Wi-Fi Aware などリンク層に暗号化がある経路でも、アプリケーション層暗号化を省略しない。
経路によって機密性の保証が変わる設計は、経路選択の自動化と両立しないからである。

## 経路情報の交換

認証済みになった直後、`channel` 上で経路候補を交換する。

```cbor
{
  "transports": ["same-lan", "wifi-aware", "wifi-direct", "ap-host", "ble"],
  "lan_addrs": ["192.168.10.4"],      現在の IP アドレス一覧
  "listen_port": 49152,               SameLanTransport の待受ポート
  "battery": { "percent": 78, "charging": false },
  "bands": ["2.4GHz", "5GHz"]
}
```

一時 AP を使う場合の SSID とパスフレーズも、この認証済みチャネル上で渡す（QR コードは v1 では使わない。BLE チャネルが既にあるため不要）。

## 禁止事項（NFR-1 の実装規約）

adapter 実装は次を行ってはならない。

- Bluetooth アダプターの電源操作、リセット、再初期化
- 既存ペアリング情報の参照以外の操作（削除、変更）
- Linux における `bluetoothd` の再起動、`hciconfig` 相当の低レベル操作
- 接続対象以外の周辺機器への GATT 接続
- 無期限の scan。scan は発見画面が前面にある間だけ行い、バックグラウンド遷移で停止する

## 後始末

次のタイミングで scan と advertise を停止する。

- GATT 交渉が完了し、Wi-Fi 系経路が確立した時点（GATT 接続自体は経路制御用に維持してよいが、v1 では切断して構わない）
- 発見画面から離れた時点
- アプリのバックグラウンド遷移、終了時

停止の実行は `Closing` 状態（`03-architecture.md`）で必ず検証される。
