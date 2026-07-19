# AirToss 設計書群

AirToss は、iOS、Android、Windows、macOS、Linux の 5 OS 間で、クラウドもアカウントも既設ルーターも前提にせず、近くの端末へファイルを直接送るアプリケーションである。
本書群は、実装エージェントが目的を見失わずに開発を進めるための設計文書である。

## 読む順序

| 文書 | 内容 |
|---|---|
| `00-vision.md` | 目的、差別化、非目標。すべての判断の基準 |
| `01-requirements.md` | 機能要件、非機能要件、対象環境 |
| `02-ux.md` | UX 原則、画面フロー、文言、エラー設計 |
| `03-architecture.md` | 全体構成。Flutter UI、Rust core、native adapter |
| `04-discovery-auth.md` | BLE による発見と認証のプロトコル |
| `05-transport.md` | 転送経路の候補、組み合わせ表、選択アルゴリズム |
| `06-transfer-protocol.md` | ファイル転送プロトコル本体 |
| `07-platform-notes.md` | OS ごとの API、制約、既知の落とし穴 |
| `08-roadmap.md` | 実装順序と、各段階の合格条件 |
| `SESSION_INTENT.md` | 認証と中断復帰で守る設計意図 |

実装前に `00` から `03` までを必ず読む。
個別の作業では、該当する `04` 以降の文書を参照する。

## 判断原則

設計書に書かれていない判断が必要になったときは、次の優先順位に従う。

1. **利用者の既存環境を壊さない**。
   Bluetooth 周辺機器、既存の Wi-Fi 接続、OS のペアリング状態に影響を与えない設計を、機能の追加より優先する。
2. **挙動の可視性**。
   内部で何をしているか（どの経路で接続し、ネットワークに何が起きるか）を利用者に見せる設計を、魔法のように隠す設計より優先する。
3. **確実に送れること**。
   最速の経路より、成功率の高い経路と、失敗時に次の一手が常にある UI を優先する。
4. **実機検証を通らない設計は採用しない**。
   OS の API 仕様上できるはずでも、実機で動くまでは設計を確定させない。検証項目は `08-roadmap.md` に定める。

原則同士が衝突したときは、番号の小さいほうを優先する。

## スコープの固定

v1 の対象は **1 対 1 の近接ファイル・テキスト転送** だけである。
イベント向けの 1 対多配布、チャンク中継、遠隔転送は v2 以降とし、v1 の設計判断に影響させない。
ただし、転送プロトコルの設計（`06`）では、将来の 1 対多への拡張を妨げる決定をしない。

## ライセンス上の制約

AirToss は MIT ライセンスで公開する。
競合の Flying Carpet は GPL-3.0 であり、そのコードの複製と改変取り込みを一切行わない。
挙動の観察と問題点の調査は行ってよいが、実装は独立に書く。

## 実装状況

`08-roadmap.md` の M0 から実装中である。現在の Rust core は次を含む。

- BLE Service Data のエンコードと検証
- GATT channel の長さプレフィックス、断片化、再構成
- X25519、HKDF-SHA256、HMAC-SHA256 による認証ハンドシェイクと 6 桁コード導出
- 5 OS 向け Flutter プロジェクト、発見画面、認証確認画面

Linux、iOS、Android の native adapter と Rust core の Flutter bridge は未実装である。M0 の合格には対応端末での実機試験が必要になる。

## 開発

Rust 1.85 以降、Flutter stable 3.44.6 以降、Java 21 を使う。

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

```sh
cd app
flutter analyze
flutter test
```

依存ライブラリは MIT と互換性のある permissive license のものに限定し、copyleft license のコードを取り込まない。
