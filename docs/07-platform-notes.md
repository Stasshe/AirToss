# 07. プラットフォーム別ノート

adapter 実装者向けの、OS ごとの API 選定と既知の制約をまとめる。
ここに書かれた API 挙動のうち「要実機検証」と付したものは、`08-roadmap.md` の該当マイルストーンで確認するまで確定扱いにしない。

## iOS

- BLE：CoreBluetooth。Peripheral（advertise + GATT Server）と Central（scan + GATT Client)の同時動作。バックグラウンドでは advertise が縮退するため、発見はフォアグラウンド前提とする
- Wi-Fi Aware：WiFiAware framework（iOS 26 以降、iPhone 12 以降）。端末選択と PIN ペアリングに DeviceDiscoveryUI を使う設計が Apple の想定だが、AirToss は発見と認証を BLE 側で完結させたい。**DeviceDiscoveryUI を経由せずにデータパスを確立できるか、または経由する場合に BLE 認証との二重確認をどう畳むかは要実機検証**。二重確認が避けられない場合、iOS ↔ Android の認証 UX だけ例外フローを許す（`02-ux.md` の原則 1 を守った上で）
- 一時 AP 参加：NEHotspotConfigurationManager。参加時に OS の確認が表示されることを UX 文言に織り込む
- Entitlement：Wi-Fi Aware、Local Network、Bluetooth の各権限とその文言を初回フローで丁寧に出す

## Android

- BLE：advertise と scan の同時動作。advertise payload は 31 バイト制限に収める
- Wi-Fi Aware：`WifiAwareManager`。`FEATURE_WIFI_AWARE` と `isAvailable` の両方を毎セッション確認し、`ACTION_WIFI_AWARE_STATE_CHANGED` を購読する。データパス確立後は `ConnectivityManager` から得たネットワークに socket を bind する
- iOS との相互運用：**NAN pairing とデータパスの相互接続が最大の検証項目**（M2）
- 一時 AP ホスト：`LocalOnlyHotspot`。SSID とパスフレーズはシステム生成のものを取得して BLE で渡す
- 一時 AP 参加：`WifiNetworkSpecifier` によるアプリスコープ接続。この方式は端末全体のデフォルトネットワークを変えないため、Android がクライアントの場合は「切り替え」でなく「並行接続」になる。UX 表示に反映する

## Windows

- BLE：WinRT `BluetoothLEAdvertisement*` と `GattServiceProvider`。adapter 実装は Rust（windows クレート）で書き、FFI 層を薄くする
- Wi-Fi Direct：WinRT `WiFiDirectDevice` / `WiFiDirectAdvertisementPublisher`。**Infrastructure 接続を維持したままの並行動作可否はドライバー依存であり、代表的なチップ（Intel AX 系など）での実機検証が必要**（M3）。事前ペアリング要求が UX に現れる場合の扱いも M3 で決める
- 一時 AP：Mobile Hotspot API（`NetworkOperatorTetheringManager`）。SSID とパスフレーズをセッション単位で設定する
- ファイアウォール：listener 起動時に受信許可のプロンプトが出る。初回起動フローで説明する

## macOS

- BLE：CoreBluetooth（iOS と共通の Swift 実装を最大限共有する）
- 高速 P2P：一般アプリから使える手段がないため、SameLan と TemporaryAp クライアントのみ
- 既知の落とし穴：一時 AP へ参加させた後、macOS がインターネット接続のある既知の Wi-Fi へ自動で戻る挙動が報告されている（Flying Carpet の既知問題）。**参加後に対象ネットワークへの固定を維持できるか要実機検証**（M4）。維持できない場合、転送中の再接続をプロトコルの再開機能（`06`）で吸収する
- Apple 端末同士：AirDrop 案内の導線を置く

## Linux

- BLE：BlueZ の D-Bus API（`org.bluez`）を通常クライアントとして使う。`hciconfig` 相当の低レベル操作、アダプター電源操作、`bluetoothd` への干渉を行わない（NFR-1）。AirToss 起動中に接続済み HID デバイスが影響を受けないことを CI 相当の手動試験項目にする（M0 合格条件）
- BlueZ の advertise は `LEAdvertisingManager1`、GATT Server は `GattManager1` で登録する
- Wi-Fi Direct：NetworkManager の P2P API（`org.freedesktop.NetworkManager` の WifiP2P）。背後は wpa_supplicant。**Windows との相互運用が業界的にも安定しておらず、M3 の検証が通るまで主経路にしない**
- 一時 AP：NetworkManager D-Bus でホットスポット接続を作成する。AP モード対応はチップ依存のため起動時に検出する
- 配布：Flatpak を第一候補とし、D-Bus と Bluetooth のポータル権限を確認する

## 共通の実装規律

- adapter は判断を持たない（`03-architecture.md` の境界規約）
- OS API のエラーは、コード、段階、生メッセージを構造化して core へ返す。UI への露出は core と Flutter が制御する
- 権限が拒否された状態を「エラー」でなく通常状態として扱い、UI は権限設定への導線を出す
