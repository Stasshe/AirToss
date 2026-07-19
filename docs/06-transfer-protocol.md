# 06. 転送プロトコル

すべての経路の `Stream` 上で同一に動く。
経路を知らないこと、および 1 対多への将来拡張を妨げないことが設計条件である。

## 接続ごとの鍵導出（Rekey）

`Stream` が確立するたび（初回接続、および途中で切れた場合の再接続すべて）、フレーム暗号化の前に鍵を新規に導出する。
`04-discovery-auth.md` で確立した `k_auth` を使い、`04` の Hello/Confirm と同型の軽量なハンドシェイクを Stream 上で行う。

```
A → B : RekeyHello { conn_id, eph_pub_A }   eph_pub は接続ごとに新規生成する X25519 一時公開鍵
B → A : RekeyHello { conn_id, eph_pub_B }

両者 : shared      = X25519(eph_priv, eph_pub_peer)
両者 : conn_transcript = SHA-256(conn_id || RekeyHello_A || RekeyHello_B)
両者 : conn_keys   = HKDF-SHA256(ikm = shared, salt = conn_transcript,
                                 info = "airtoss v1 conn")
        → k_send_A_conn, k_send_B_conn

A → B : SessionBind { mac: HMAC(k_auth, "bind-a" || conn_transcript) }
B → A : SessionBind { mac: HMAC(k_auth, "bind-b" || conn_transcript) }
```

`SessionBind` の MAC 検証は、この接続が同一セッション（同一の `k_auth`）の当事者どうしのものであることを保証する。
`conn_id` は接続ごとに送信側が新規生成する乱数で、同一セッション内での接続の使い回しや古い `conn_transcript` の再送を防ぐ。

## フレーミングと暗号化

ワイヤ上の単位は暗号化フレームである。

```
frame := len (u32 BE) || ciphertext
ciphertext := ChaCha20-Poly1305(key = k_send_self_conn,
                                nonce = direction_byte || counter (u64 BE) を 12 bytes に整形,
                                aad = "",
                                plaintext)
```

- 方向別鍵 `k_send_A_conn`、`k_send_B_conn` は、その `Stream` 専用の鍵であり、接続が切れて再接続すれば必ず作り直す
- nonce カウンターは方向ごとに、その接続の開始時点で 0 から単調増加する。鍵が接続ごとに新規のため、異なる接続の間で同じ (鍵, nonce) の組が生じることは構造上ない
- 平文は `type (u8) || CBOR payload`

TLS を使わない理由は、鍵配送が BLE ハンドシェイクと接続ごとの Rekey で完了しており、証明書基盤もセッション再開も不要なためである。
暗号プリミティブは RustCrypto の監査済みクレートを使い、自作しない。

## メッセージ

RekeyHello は `channel` と同様の長さプレフィックス付き CBOR で、暗号化フレームの外側（鍵確立前）に流れる。
SessionBind 以降のメッセージはすべて上記の暗号化フレームに乗る。

| type | 名前 | 方向 | payload |
|---|---|---|---|
| 0x01 | SessionBind | 双方向 | `{ mac: HMAC(k_auth, "bind-<a\|b>" \|\| conn_transcript) }` |
| 0x02 | Offer | 送信側 | `{ transfer_id, items: [{ item_id, kind, name, size, sha256 }] }` |
| 0x03 | Answer | 受信側 | `{ transfer_id, accept: bool, resume: [{ item_id, offset }] }` |
| 0x04 | Chunk | 送信側 | `{ item_id, offset, data }` |
| 0x05 | ItemDone | 送信側 | `{ item_id }` |
| 0x06 | ItemResult | 受信側 | `{ item_id, ok: bool, reason? }` |
| 0x07 | TransferDone | 送信側 | `{ transfer_id }` |
| 0x08 | Text | 双方向 | `{ text }`（テキスト、URL の即時共有） |
| 0x09 | Abort | 双方向 | `{ transfer_id?, reason }` |
| 0x0A | Ping / 0x0B Pong | 双方向 | `{}`（無通信 15 秒で送出、30 秒で切断判定） |

`kind` は `file`、`directory` のいずれか。
ディレクトリは相対パス付きのファイル列に展開して items に並べる（パス区切りは `/` に正規化し、`..` を含むパスは受信側で拒否する）。

## 転送の流れ

```
接続確立
→ RekeyHello 交換（接続専用の鍵を導出）
→ SessionBind 交換（MAC 不一致なら即切断）
→ Offer
→ 受信側 UI で許可
→ Answer { accept: true }
→ Chunk の連続送出（item ごとに offset 順、並列送出しない）
→ ItemDone → 受信側がハッシュ検証 → ItemResult
→ 全 item 完了後 TransferDone
→ セッションは Connected のまま。次の Offer を双方向に送れる
```

チャンクサイズは 1 MiB を基準とし、BleGatt 経路では 4 KiB に落とす（経路確立時に core が指示する。プロトコル自体は size に依存しない）。

## 再開

受信側は item ごとに、連続して書き込めた末尾オフセットを保持する。
Stream が切れて再接続した場合、新しい Stream 上でまず Rekey と SessionBind をやり直してから（`k_auth` はセッション終了まで生きているため BLE への再接続は不要）、送信側は同じ `transfer_id` で Offer を再送し、受信側は `resume` に各 item の継続オフセットを入れて返す。
送信側はそのオフセットから Chunk を再開する。

ビットマップ方式を採らない理由は、v1 の送出が item ごとの逐次であり、穴あきが構造上生じないためである。
1 対多と並列化を導入する v2 で再設計する（そのために offset を Chunk に明示しており、逐次前提はワイヤ形式に焼き込まれていない）。

## 検証

- item 単位：受信完了時に SHA-256 を Offer の値と照合し、ItemResult で返す
- 不一致の item は自動で 1 回だけ再転送を試み、再度失敗したら UI へ通知する

## 受信ファイルの配置

- 検証が通るまで、一時ファイル名（`.airtoss-partial` 拡張子）で保存先ディレクトリに書く
- 検証成功後に本来の名前へ rename する
- 名前衝突は `name (2).ext` 方式で回避し、既存ファイルを上書きしない
- 保存先は OS 標準のダウンロード相当ディレクトリを既定とし、設定で変更できる
