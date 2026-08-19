## 現状決まっていること

- TUIとブラウザの両方で動くツールを作る
- 開発順序は 設計 → テストコード → 実装 で固定する

## 要件

- ローカルマシンのshell環境を使える
  - iTermがあれば、そのままローカルでshellサーバーに直接アタッチして使う(ネットワーク越しにしない)
  - iTermが無い環境ではブラウザから同じshell環境にリモートアクセスできる
- リモート接続(ブラウザ経由)は認証を必須にする(誰でも繋がる状態にしない)
- 文字サイズの拡縮はMVPでは見送り(ショートカットキー衝突の懸念があり優先度低。必要になれば後から追加)

## アーキテクチャ

- 実shellのPTYをネットワーク越しに繋ぐ方式(ttyd/gotty型)を採用。ratatuiウィジェットをWASM描画するratzilla方式は、shell環境そのものを使うという要件と合わないため不採用
- 言語: Rust
- ブラウザ側ターミナル描画: xterm.js
- リモート公開: Cloudflare Tunnel(cloudflared)。ポート開放・TLS証明書管理なしで公開URLを持てる
- 認証: Cloudflare Access(Zero Trust)、メールOTP。自前でトークン/パスワード管理は作らない
- ローカル(iTerm)接続はCloudflareを経由しない直結

## 構成要素(想定)

- server: Rustプロセス。PTYでローカルshellを起動し、ローカルソケット/WebSocketでクライアントに入出力を中継する
- iTermクライアント: serverにローカルアタッチしてそのまま表示(素のPTY接続、認証不要)
- webクライアント: xterm.js + WebSocket。Cloudflare Tunnel/Access経由でのみ到達可能

## 次にやること

- テストコード作成(PTY起動・入出力中継・WebSocketブリッジの単体テストから)
- Cargoプロジェクト初期化
