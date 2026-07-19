# 03. アーキテクチャ

## 全体構成

3 層に分ける。
UI は Flutter、プロトコルとロジックは Rust、OS 固有の無線 API は native adapter が担う。

```
Flutter (Dart)
├─ 端末一覧、認証、経路選択、転送進捗、エラー UI
└─ 状態表示のみ。プロトコル判断を持たない

        │ flutter_rust_bridge

Rust core（全 OS 共通、単一クレート群）
├─ session      セッション状態機械
├─ crypto       X25519、HKDF、ChaCha20-Poly1305、確認コード導出
├─ protocol     転送プロトコルのフレーミングとメッセージ
├─ chunking     分割、再開、SHA-256 検証
├─ routing      経路候補の評価と選択
└─ transport    Transport 抽象と LAN 実装

        │ platform channel / FFI

Native adapters（OS 固有の最小実装）
├─ iOS      Swift   CoreBluetooth, WiFiAware, Network.framework
├─ Android  Kotlin  BluetoothLE, WifiAwareManager, LocalOnlyHotspot
├─ macOS    Swift   CoreBluetooth
├─ Windows  Rust    WinRT (Bluetooth LE, WiFiDirect, Mobile Hotspot)
└─ Linux    Rust    BlueZ D-Bus, NetworkManager D-Bus
```

## 各層の選定理由

**Flutter を UI に使う理由**。
この製品は 5 OS で同一の画面フローと文言を保つことが差別化の一部であり、UI を OS ごとに書くと文言とフローの統一が崩れやすい。
モバイル側の権限ダイアログ、ファイルピッカー、Share Sheet 連携も Flutter のプラグイン基盤に乗せられる。

**Rust core にプロトコル全体を置く理由**。
発見後の挙動（認証、経路選択、転送、再開、検証）が OS ごとに揺れると、相互運用の検証コストが組み合わせ数で増える。
無線 API の呼び出し以外をすべて共通コードにすることで、OS 間の差異を adapter の境界に閉じ込める。

**adapter を native に書く理由**。
BLE、Wi-Fi Aware、Wi-Fi Direct は各 OS の公式 API 以外に安定した入口がない。
サードパーティのクロスプラットフォーム BLE ライブラリに製品の根幹を預けない（Flying Carpet の失敗の一因が Linux BLE ライブラリの制約にあった）。

## Transport 抽象

Rust core は経路を trait として扱う。

```rust
trait Transport {
    /// この経路がいま利用可能かを返す（実行時能力の検出を含む）
    async fn probe(&self, peer: &PeerInfo) -> Availability;

    /// 経路を確立し、双方向バイトストリームを返す
    async fn connect(&self, peer: &PeerInfo) -> Result<Box<dyn Stream>>;

    /// 経路確立に必要な待受を開始する
    async fn listen(&self) -> Result<Box<dyn Listener>>;

    /// 両端末のネットワーク影響を返す（UX の事前明示に使う）
    fn network_impact(
        &self,
        peer: &PeerInfo,
        role: TransportRole,
    ) -> NetworkImpact;
}
```

実装は次の 5 つ。
`Transport` trait の実装（`probe`/`connect`/`listen`/`network_impact`）は、経路の種類によらずすべて Rust core 側（`transport` モジュール）に置く。
adapter は trait を実装しない。adapter が公開するのは無線操作の実行だけであり（`adapter 境界の規約`）、core 側の Transport 実装がそれを呼び出して `Stream`/`Listener` に包む。

```
SameLanTransport        Rust core 内で完結（TCP）
WifiAwareTransport      core が adapter にデータパス確立を指示し、返された socket を Stream に包む
WifiDirectTransport     同上（Windows, Linux）
TemporaryApTransport    core が adapter に AP 作成/参加を指示し、以後は TCP を core が扱う
BleGattTransport        core が adapter の GATT 読み書きを Stream として包む
```

新しい経路を追加するときは、core に `XxxTransport` を 1 つ追加し、その内部から呼ぶ無線操作だけを adapter に生やす。判断・状態管理を adapter 側に持たせない。

転送プロトコル（`06`）は `Stream` の上でのみ動き、経路を知らない。
これにより、経路の追加と削除が転送ロジックに波及しない。

## 状態機械

セッションは Rust core が単一の状態機械として管理し、Flutter はその状態を購読して描画する。

```
Idle
 → Discovering        （BLE scan/advertise 中）
 → Negotiating        （GATT 接続、鍵交換）
 → AwaitingUserVerify （確認コード表示中）
 → Routing            （経路 probe と選択）
 → AwaitingRouteChoice（切り替えを伴う場合のみ）
 → Establishing       （経路確立）
 → Connected          （転送プロトコル稼働。複数転送を往復できる）
 → Closing            （後始末：BLE 停止、一時ネットワーク破棄）
 → Idle
```

どの状態からもエラーで `Failed(reason, recoverable_actions)` へ遷移でき、UI はそこから `02-ux.md` のエラー画面を構成する。
後始末（`Closing`）は失敗経路でも必ず通す。
これが NFR-5（無線動作を残さない）の実装点になる。

`NetworkImpact` は local と peer のそれぞれについて `Preserved` または `Switches` を持つ。TemporaryAp ではホスト側、クライアント側、Android のアプリスコープ接続で結果が異なるため、経路種別だけから固定値を返さない。

`Connected` で同時に実行する転送は 1 つだけとする。完了後は同じセッション内でどちらの端末からも次の転送を開始できる。

## 進行中セッションの復元

プロセス終了後の再開に必要な `session_device_id`、`k_auth`、転送識別子、item メタデータ、連続書き込み済み offset を、進行中の 1 転送に限ってアプリ専用領域へ保存する。再起動後は同じセッションとして経路を再確立し、接続ごとの Rekey と SessionBind をやり直す。完了、拒否、中止時に復元状態を削除する。転送履歴には使わない。

## adapter 境界の規約

adapter は「無線操作の実行者」であり、判断を持たない。

- adapter が公開するのは、advertise 開始/停止、scan 開始/停止、GATT 接続、経路確立、能力照会のみ
- リトライ、タイムアウト、経路の選び直しは Rust core が指示する
- adapter は OS のエラーを構造化して core へ返し、自ら回復を試みない（回復手段の選択は UX に関わるため core と UI の責務）

この境界を守ることで、NFR-1（周辺機器無影響）の検証を adapter 単体テストに落とせる。
