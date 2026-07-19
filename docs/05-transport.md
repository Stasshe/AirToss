# 05. 転送経路

## 経路候補

優先順位の高い順に並べる。
上にあるほど「既存ネットワークを維持し、かつ速い」。

| # | 経路 | 対応 | ネットワーク影響 |
|---|---|---|---|
| 1 | SameLan（TCP） | 全 5 OS | なし |
| 2 | WifiAware | iOS ↔ iOS、iOS ↔ Android、Android ↔ Android | なし |
| 3 | WifiDirect（並行） | Windows、Linux の相互 | なし（並行動作対応チップに限る） |
| 4 | TemporaryAp | ホスト：Windows、Linux、Android。クライアント：全 OS | ホストは維持。クライアントは原則切り替え、Android は維持 |
| 5 | BleGatt | 全 5 OS | なし（低速） |

macOS は WifiAware も WifiDirect も一般アプリから使えず、AP ホストの公開 API も製品要件にできない。
したがって macOS は、SameLan、TemporaryAp のクライアント、BleGatt の 3 つで参加する。
Apple 端末同士の転送で AirDrop のほうが適切な場面では、UI から AirDrop を案内してよい（自前経路を無理に使わせない）。

## 組み合わせ表

同一 LAN にいない場合の主経路。

| | iOS | Android | Windows | macOS | Linux |
|---|---|---|---|---|---|
| **iOS** | WifiAware | WifiAware | TempAp(Win host) | (AirDrop 案内) | TempAp(Linux host) |
| **Android** | | WifiAware | TempAp(Android/Win host) | TempAp(Android host) | TempAp(Android/Linux host) |
| **Windows** | | | WifiDirect | TempAp(Win host) | WifiDirect |
| **macOS** | | | | (AirDrop 案内) | TempAp(Linux host) |
| **Linux** | | | | | WifiDirect |

TempAp を含むセルでは、必ず経路選択画面（`02-ux.md`）を経由し、実行時に成立するホスト方向と BleGatt だけを提示する。高速 2 方向を固定表示しない。
WifiDirect（並行）は v1 では Windows ↔ Linux の PC 相互のみを候補にする（経路候補一覧の `#3` と一致させている）。
Android の Wi-Fi P2P スタックは Windows/Linux の並行動作検証（`08-roadmap.md` M3）の対象外であり、Android ↔ Windows/Linux の並行 WifiDirect は v1 のスコープに含めない（v2 で改めて検証項目を立てて候補に上げるかを判断する）。
そのため Android ↔ Windows/Linux は常に TempAp を経由する。

## 選択アルゴリズム

BLE 交渉で得た能力情報と、実行時 probe の結果から決める。

```
1. SameLan probe:
   BLE で交換した lan_addrs の各アドレスへ、listen_port に TCP 接続を試行（タイムアウト 800 ms）
   ─ 成功 → SameLan で確定。以降の手順を省略
   （mDNS は使わない。BLE で正確なアドレスを交換済みのため、到達性の直接確認が最も速く確実）

2. 組み合わせ表から無影響経路（WifiAware / WifiDirect）を引く:
   両端末の capability_flags が対応を示す場合のみ候補にする
   ─ 確立成功 → 確定
   ─ 確立失敗 → 3 へ（失敗理由は記録し「詳細」に出す）

3. TemporaryAp が可能な組み合わせ:
   → 経路選択画面を表示（成立する各ホスト方向 + BleGatt 低速の選択式）
   → 利用者の承諾後に確立

4. いずれも不可:
   → BleGatt を提示（サイズ上限の警告付き）
```

ホスト決定（TempAp と WifiDirect の Group Owner）は、両端末が能力を持つ場合、次の順で機械的に決める。

```
1. 電源接続中の端末
2. 5 GHz 帯対応の端末
3. バッテリー残量の多い端末
4. session_device_id の辞書順
```

## 各経路の実装要点

**SameLan**。
Rust core の TCP 実装で完結する。
listener はセッション開始時に起動し、`Closing` で閉じる。
QUIC は v1 では使わない（アプリケーション層暗号化を自前で持つため、TCP との差分が性能最適化に限られる。v2 で再検討）。

**WifiAware**。
iOS は DeviceDiscoveryUI と Network.framework、Android は WifiAwareManager のデータパスを adapter が確立し、得られた socket を core へ渡す。
Android は `FEATURE_WIFI_AWARE` と `isAvailable` を毎セッション確認する。
テザリングや Wi-Fi Direct 使用中に一時的に不可となる端末があるため、`isAvailable` の変化を購読し、不可の間は capability_flags から落とす。

**WifiDirect（並行）**。
要件は「既存の Infrastructure 接続を維持したまま」の接続である。
Windows は WinRT の WiFiDirect API、Linux は NetworkManager の P2P API（背後は wpa_supplicant）を使う。
並行動作可否はチップ依存のため、adapter が起動時に検出して capability_flags に反映する。
検出できない、または並行不可の場合、この経路は候補から外し、TempAp へ落とす（既存接続を切ってまで WifiDirect を張らない。それは TempAp と同じ影響を持ちながら UX 上の説明が難しくなるだけである)。

**TemporaryAp**。
ホスト側 adapter が SSID とパスフレーズを乱数生成し、認証済み BLE チャネルで相手へ渡す。
クライアント側は、iOS では NEHotspotConfigurationManager、Android では WifiNetworkSpecifier、macOS と他 PC では OS の Wi-Fi 参加 API を使う。
経路候補のネットワーク影響は端末ごとに算出する。ホスト側は既存接続を維持し、通常のクライアント側は切り替わる。Android の WifiNetworkSpecifier による参加はアプリスコープの並行接続なので維持とする。
転送完了後、ホストは AP を破棄し、クライアント adapter は元のネットワークへの復帰を確認する。
復帰確認までが `Closing` の完了条件である。

**BleGatt**。
`channel` とは別の characteristic を追加せず、認証済みチャネル上で転送プロトコル（`06`）をそのまま流す。
スループットは数十 kB/s 程度しか期待できないため、10 MB を超える転送では所要時間の見積もりを表示して確認を取る。

## 経路確立後

確立した `Stream` では、転送プロトコルの Rekey ハンドシェイクと `SessionBind`（`06`）を最初に行う。
`Stream` ごとに新しい暗号鍵を導出したうえで、BLE ハンドシェイクで得た長期鍵 `k_auth` による所有証明を行う。
これにより、経路の乗っ取り（別の端末が同じ AP に参加して接続してくるなど）を排除すると同時に、再接続のたびに鍵が更新されるため経路をまたいだ鍵の使い回しが生じない。
