# lib_bbev

ATA / SCSI コマンドの組み立て、SAT変換、Linux SG_IOでの送信を扱うRustライブラリのPoC。

ATA / SCSI側がコマンド情報を公開し、I/O engineがその情報を取り出して送信する構成です。
SG_IO以外のengineでも、ATAのフィールドやFIS、SCSIのCDBと転送情報を利用できます。

## 構成

| モジュール | 役割 |
| --- | --- |
| `ata` | ATAコマンド・protocol・応答の表現、H2D FIS生成 |
| `ata::gpl` | General Purpose LoggingのDMA / FPDMA読み書きコマンド生成 |
| `scsi` | CDB・転送情報・status・senseの表現とsense解析 |
| `scsi::sat` | ATA PASS-THROUGH (12)/(16)/(32)生成、SCSI応答からATA応答への変換 |
| `blk_cmd` | 共通の転送方向・長さの型 |
| `sg_io` | Linux SG_IOによる送信、バッファ確保・検証 |

## ビルドとテスト

Rust 2024 editionを使用しています。SG_IOの実行にはLinuxが必要です。

```sh
cargo build
cargo test
cargo build --example gpl
```

テストはデバイスへ送信せずに実行できます。

## GPLログを読む

`gpl` exampleはREAD LOG DMA EXTでログを読み取り、xxd風に表示します。
対象デバイスをread/writeで開ける権限が必要です。

```sh
./target/debug/examples/gpl /dev/sda -a 0x00 -n 0 -c 1
./target/debug/examples/gpl --help
```

| 引数 | 意味 | デフォルト |
| --- | --- | --- |
| `<DEVICE>` | デバイスファイル（例: `/dev/sda`、`/dev/sg0`） | 必須 |
| `-a`, `--address` | ログアドレス | `0` |
| `-n`, `--number` | 開始ページ番号 | `0` |
| `-c`, `--count` | 読み取るページ数 | `1` |

数値は10進数と`0x` / `0X`付き16進数に対応しています。
1ページは512バイト。exampleの転送上限は1024 KiBです。
表示は1行16バイトで、左から取得データ内のオフセット、2バイト区切りの16進数、ASCIIを並べます。

```text
00000000: 0001 0203 0405 0607 0809 0a0b 0c0d 0e0f  ................
00000010: 1011 1213 1415 1617 1819 1a1b 1c1d 1e1f  ................
```

## コマンド情報を使う

`ata::Command::builder()`でレジスタ値とコマンドを直接指定できます。
以下はLBA `0x1234`から1セクタを読むREAD DMA EXTの例です。

```rust
use lib_bbev::ata::{Command, CommandCode};
use lib_bbev::blk_cmd::XferLength;

let ata = Command::builder()
    .command(CommandCode::ReadDmaExt)
    .lba(0x1234)
    .count(1)
    .device(0x40) // LBAモード
    .dma_data_in(XferLength::Sectors(1));
```

最後に`.nondata()`、`.pio_data_in(...)`、`.dma_data_in(...)`などでprotocolと転送長を選ぶと、
`ata::Command`が返ります。`count`はATAレジスタ値、`XferLength`はengineが使う転送長なので、
コマンドに合わせて両方を指定します。`Sectors`のバイト数はデバイスのセクタサイズに依存します。

GPL用のビルダーも同じ`ata::Command`を返します。
どちらで組み立てても、公開フィールドやFISの参照、SAT変換は同じように行えます。

```rust
use lib_bbev::ata::gpl::Gpl;
use lib_bbev::scsi::sat::AtaPt;

let ata = Gpl::builder()
    .address(0x10)
    .page_number(0)
    .page_count(1)
    .read_log_dma_ext();

// ATA engineでは公開フィールド・protocol、またはFISを使う。
let lba = ata.lba;
let fis = ata.fis();

// SCSI engineではSATで変換したCDBと転送情報を使う。
let sat = AtaPt::new(ata);
let scsi = sat.cdb16();
let cdb = scsi.cdb.as_bytes();
let transfer = scsi.xfer;
```

バッファは呼び出し側が所有し、SG_IOでは`Device::allocate()`と`Device::execute()`で扱います。

`Device::execute()`は`Result<scsi::Response, SgIoError>`を返します。
CHECK CONDITIONやBUSYも`Ok(Response)`なので、コマンドの成否は`response.status`で確認します。
送信やバッファ検証の失敗は`Err`になります。

`AtaPt::response()`はSCSI応答から`Option<ata::Response>`を取り出します。
GOOD応答にはATAレジスタ情報が含まれないため`None`になります。
fixed形式で上位COUNT / LBAを復元できない場合は、STATUS / ERRORを保持して`register: None`を返します。

## SG_IOで送信する

GPLログの読み取りを、コマンド生成から送信まで行う例です。
`Gpl::builder()`の部分は、前述の`ata::Command::builder()`で作ったコマンドにも置き換えられます。

```rust
use lib_bbev::ata::gpl::Gpl;
use lib_bbev::scsi::sat::AtaPt;
use lib_bbev::scsi::Status;
use lib_bbev::sg_io::Device;
use std::error::Error;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;

fn main() -> Result<(), Box<dyn Error>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/sda")?;
    // セクタサイズ512バイト、転送上限1024 KiB。
    let device = Device::new(file.as_raw_fd(), 512, 1024);

    let ata = Gpl::builder()
        .address(0x00)
        .page_number(0)
        .page_count(1)
        .read_log_dma_ext();
    let sat = AtaPt::new(ata);
    let scsi = sat.cdb16();
    let mut buffer = device.allocate(&scsi)?;
    let response = device.execute(&scsi, &mut buffer)?;

    if let Some(ata_response) = sat.response(&response) {
        eprintln!(
            "ATA STATUS={:02x}, ERROR={:02x}",
            ata_response.status, ata_response.error
        );
    }
    if response.status != Status::Good {
        return Err(format!("SCSI command failed: {response:?}").into());
    }

    // 読み取りデータはbufferに入る。ここでは先頭16バイトを表示する。
    println!("{:02x?}", &buffer[..16]);
    Ok(())
}
```

`Device`はfdを所有しないため、送信が終わるまで`file`を保持します。
引数処理とxxd風の表示を含む例は[examples/gpl.rs](examples/gpl.rs)を参照してください。

## 設計・仕様メモ

- [ARCHITECTURE.md](ARCHITECTURE.md): モジュールの責務・依存関係・データの流れ
- [SAT.md](SAT.md): SAT-4のフィールド配置・変換規則
- [GPL.md](GPL.md): GPLコマンドのフィールド配置
