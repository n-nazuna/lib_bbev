# General Purpose Logging (GPL) 要約

ACS-4準拠。

参照先：

- [§1 ログの指定・LBA配置](#gpl-address)
- [§2 READ / WRITE LOG EXT・DMA EXT](#gpl-dma)
- [§3 NCQカプセル化](#gpl-ncq)
- [§4 制約・エラー](#gpl-errors)

<a id="gpl-address"></a>

## 1 ログの指定・LBA配置

`[上位bit:下位bit]` はフィールド内のビット範囲。
PAGE NUMBERは開始ページ番号、LOG ADDRESSは対象ログを指定する。
General Purpose Log Directory（LOG ADDRESS=`0x00`）は各ログのサイズを512バイトページ数で示す。

| Bits | Field name | Value |
| --- | --- | --- |
| LBA [47:40] | Reserved | `0x00` |
| LBA [39:32] | PAGE NUMBER [15:8] | 開始ページ番号の上位バイト |
| LBA [31:16] | Reserved | `0x0000` |
| LBA [15:8] | PAGE NUMBER [7:0] | 開始ページ番号の下位バイト |
| LBA [7:0] | LOG ADDRESS | ログアドレス |

例：LOG ADDRESS=`0x12`、PAGE NUMBER=`0x3456` → LBA=`0x003400005612`。

<a id="gpl-dma"></a>

## 2 READ / WRITE LOG EXT・DMA EXT

| Command | COMMAND | Protocol | 転送方向 | GPLでのサポート |
| --- | --- | --- | --- | --- |
| READ LOG EXT | `0x2F` | PIO Data-In | デバイス→ホスト | 必須 |
| WRITE LOG EXT | `0x3F` | PIO Data-Out | ホスト→デバイス | 必須 |
| READ LOG DMA EXT | `0x47` | DMA | デバイス→ホスト | 任意 |
| WRITE LOG DMA EXT | `0x57` | DMA | ホスト→デバイス | 任意 |

いずれも48bitコマンド。DMA版は [§3](#gpl-ncq) の方法でNCQ環境でも処理できる。

| Field name | Read | Write |
| --- | --- | --- |
| FEATURE [15:0] | ログが定義する値。定義がなければReserved（`0x0000`） | Reserved（`0x0000`） |
| COUNT [15:0] | 読み出すLOG PAGE COUNT | 書き込むLOG PAGE COUNT |
| LBA [47:0] | [§1](#gpl-address) | [§1](#gpl-address) |
| COMMAND [7:0] | 上表のCOMMAND | 上表のCOMMAND |

転送バイト数は `LOG PAGE COUNT × 512`。LOG PAGE COUNTの`0`は無効。

**DEVICE**（4コマンド共通）

| Bits | Field name | Value |
| --- | --- | --- |
| 7, 5 | Obsolete | Obsolete |
| 6 | N/A | N/A |
| 4 | Transport Dependent | トランスポート依存 |
| 3:0 | Reserved | `0` |

<a id="gpl-ncq"></a>

## 3 NCQカプセル化

| Command | COMMAND | SUBCOMMAND | カプセル化するコマンド | Protocol | 転送方向 |
| --- | --- | --- | --- | --- | --- |
| RECEIVE FPDMA QUEUED | `0x65` | `0x01` | READ LOG DMA EXT | DMA Queued | デバイス→ホスト |
| SEND FPDMA QUEUED | `0x64` | `0x02` | WRITE LOG DMA EXT | DMA Queued | ホスト→デバイス |

どちらのGPLサブコマンドもsequential処理を要求する。

| Field name | RECEIVE（Read） | SEND（Write） |
| --- | --- | --- |
| FEATURE [15:0] | 転送する512バイトブロック数 | 転送する512バイトブロック数 |
| COUNT [15:0] | 下表 | 下表 |
| LBA [47:0] | READ LOG DMA EXTのLBAを保持 | WRITE LOG DMA EXTのLBAを保持 |
| AUXILIARY [15:0] | READ LOG DMA EXTのFEATURE | Reserved（`0x0000`） |
| AUXILIARY [31:16] | Reserved（`0x0000`） | Reserved（`0x0000`） |
| DEVICE [7:0] | `0x40` | `0x40` |

RECEIVE / SEND FPDMA QUEUED自体のFEATURE定義では、`0x0000`は65,536ブロックを表す。
LOG PAGE COUNTの制約は [§2](#gpl-dma) を参照。

**COUNT**

| Bits | Field name | Value |
| --- | --- | --- |
| 15:14 | PRIO | NCQ優先度 |
| 13 | Reserved | `0` |
| 12:8 | SUBCOMMAND | Read=`0x01`、Write=`0x02` |
| 7:3 | NCQ TAG | コマンドタグ（5bit） |
| 2:0 | Reserved | `0` |

**DEVICE**

| Bits | Field name | Value |
| --- | --- | --- |
| 7 | Reserved | `0` |
| 6 | 固定値 | `1` |
| 5 | Reserved | `0` |
| 4 | 固定値 | `0` |
| 3:0 | Reserved | `0` |

<a id="gpl-errors"></a>

## 4 制約・エラー

以下の場合、デバイスはcommand abortedを返す。

- LOG PAGE COUNTが`0`、またはその他の入力フィールドが無効。
- 対象ログのfeature setが未サポート、または無効。
- 読み出しでは、別途規定がない限り、PAGE NUMBER + LOG PAGE COUNTがログサイズを超える。
- 書き込みでは、PAGE NUMBER + LOG PAGE COUNTがログサイズを超える、または対象ログが読み出し専用。

Reservedフィールドは`0`にする。
