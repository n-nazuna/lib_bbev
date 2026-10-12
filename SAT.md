# SAT-4 ATA PASS-THROUGH 要約

SAT-4準拠。

参照先：

- [§1 概要](#sat-overview)
- [§2 CONTROL](#sat-control)
- [§3 CDB (12)・共通フィールド](#sat-cdb12)
- [§4 CDB (16)・EXTEND](#sat-cdb16)
- [§5 CDB (32)](#sat-cdb32)
- [§6 ATAフィールドへの対応・転送長](#sat-translations)
- [§7 完了状態](#sat-status)
- [§8 ATA Status Return descriptor](#sat-descriptor)

<a id="sat-overview"></a>

## 1 ATA PASS-THROUGH commands overview

ATAコマンド、データ、完了状態をSATL経由で受け渡す。
ATA PASS-THROUGH をサポートするSATLは、[§8](#sat-descriptor) の ATA Status Return descriptor もサポートする。

`[上位bit:下位bit]` はフィールド内のビット範囲。Byte は0始まり。

<a id="sat-control"></a>

## 2 CONTROL byte

CONTROL は SCSI CDB の制御フィールド。ATA Control レジスタへの対応は定義されていない。

| Field name | 規定 |
| --- | --- |
| Vendor specific | ベンダ固有の用途に使用してよい |
| NACA | 1なら CHECK CONDITION、ILLEGAL REQUEST / INVALID FIELD IN CDB |

<a id="sat-cdb12"></a>

## 3 ATA PASS-THROUGH (12) command

| Bytes | Field name | Value |
| ---: | --- | --- |
| 0 | OPERATION CODE | `0xA1` |
| 1 | PROTOCOL | `PROTOCOL << 1` |
| 2 | 転送フラグ | [共通ビット配置](#sat-transfer-flags) |
| 3 | FEATURES [7:0] | 指定値 |
| 4 | COUNT [7:0] | 指定値 |
| 5 | LBA [7:0] | 指定値 |
| 6 | LBA [15:8] | 指定値 |
| 7 | LBA [23:16] | 指定値 |
| 8 | DEVICE | 指定値 |
| 9 | COMMAND | ATA opcode |
| 10 | Reserved | `0x00` |
| 11 | CONTROL | [§2](#sat-control) |

(16)/(32) でも共通フィールドに同じ定義を使う。

### 3.1 PROTOCOL バイト

| Bit | (12) / (16) Byte 1 | (32) Byte 10 |
| --- | --- | --- |
| 7:5 | Obsolete | Reserved |
| 4:1 | PROTOCOL | PROTOCOL |
| 0 | (12): Reserved / (16): EXTEND | EXTEND |

<a id="sat-protocol-values"></a>

**PROTOCOL field**（シフト前の4bit値）

| Value | Description |
| --- | --- |
| `0x0` | Device Management - ATA hardware reset |
| `0x1` | Device Management - ATA software reset |
| `0x2` | Reserved |
| `0x3` | Non-Data |
| `0x4` | PIO Data-In |
| `0x5` | PIO Data-Out |
| `0x6` | DMA |
| `0x7` | Reserved |
| `0x8` | Execute Device Diagnostic |
| `0x9` | Non-data command - Device Reset |
| `0xA` | UDMA Data In |
| `0xB` | UDMA Data Out |
| `0xC` | NCQ |
| `0xD`〜`0xE` | Reserved |
| `0xF` | Return Response Information |

`0x0`/`0x1` では PROTOCOL と OFF_LINE のみ有効。`0xF` では PROTOCOL のみ有効で、
現在のATA Command Blockを読み出して ATA Status Return descriptor に返す。

<a id="sat-transfer-flags"></a>

### 3.2 転送フラグバイト

(12)/(16) は Byte 2、(32) は Byte 11。

| Bits | Field name | Value |
| --- | --- | --- |
| 7:6 | OFF_LINE | `00`=0秒、`01`=2秒、`10`=6秒、`11`=14秒（ATA STATUS の有効化待ち） |
| 5 | CK_COND | `0`=エラー時のみ CHECK CONDITION、`1`=正常完了でも CHECK CONDITION と完了情報を返す |
| 4 | T_TYPE | `0`=512バイトブロック、`1`=ATA論理セクタ |
| 3 | T_DIR | `0`=アプリケーションクライアント→ATAデバイス、`1`=ATAデバイス→アプリケーションクライアント |
| 2 | BYTE_BLOCK | `0`=バイト単位、`1`=ブロック単位 |
| 1:0 | T_LENGTH | 転送長の参照元（下表） |

<a id="sat-transfer-length"></a>

**T_LENGTH field**

| T_LENGTH | 転送長の参照元 |
| --- | --- |
| `00` | 無転送 |
| `01` | FEATURES |
| `10` | COUNT |
| `11` | TPSIU（トランスポート固有の情報単位） |

<a id="sat-transfer-units"></a>

**BYTE_BLOCK・T_TYPE・T_LENGTH の対応**

| BYTE_BLOCK | T_TYPE | T_LENGTH | 転送長が表す単位 |
| --- | --- | --- | --- |
| `1` | `0` | 非ゼロ | 512バイトブロック数 |
| `1` | `1` | 非ゼロ | ATA論理セクタ数 |
| `0` | 任意 | 非ゼロ | バイト数 |
| 任意 | 任意 | `00` | 無転送 |

PIO と T_DIR の方向が不一致なら CHECK CONDITION、ILLEGAL REQUEST / INVALID FIELD IN CDB。
無転送時は T_DIR を無視する。SATL は共通フィールドで転送を決め、COMMAND はATA COMMANDへのコピー以外に使用しない。

<a id="sat-cdb16"></a>

## 4 ATA PASS-THROUGH (16) command

| Bytes | Field name | Value |
| ---: | --- | --- |
| 0 | OPERATION CODE | `0x85` |
| 1 | PROTOCOL / EXTEND | `(PROTOCOL << 1) \| EXTEND` |
| 2 | 転送フラグ | [共通ビット配置](#sat-transfer-flags) |
| 3 | FEATURES [15:8] | 指定値 |
| 4 | FEATURES [7:0] | 指定値 |
| 5 | COUNT [15:8] | 指定値 |
| 6 | COUNT [7:0] | 指定値 |
| 7 | LBA [31:24] | 指定値 |
| 8 | LBA [7:0] | 指定値 |
| 9 | LBA [39:32] | 指定値 |
| 10 | LBA [15:8] | 指定値 |
| 11 | LBA [47:40] | 指定値 |
| 12 | LBA [23:16] | 指定値 |
| 13 | DEVICE | 指定値 |
| 14 | COMMAND | ATA opcode |
| 15 | CONTROL | [§2](#sat-control) |

EXTEND=0 の場合、SATLは FEATURES [15:8]、COUNT [15:8]、LBA [31:28]、
LBA [39:32]、LBA [47:40] を無視し、(12) と同様に処理する。
EXTEND=1 では [§6](#sat-translations) の48bit形式を使う。

<a id="sat-cdb32"></a>

## 5 ATA PASS-THROUGH (32) command

| Bytes | Field name | Value |
| ---: | --- | --- |
| 0 | OPERATION CODE | `0x7F` |
| 1 | CONTROL | [§2](#sat-control) |
| 2〜6 | Reserved | 全て `0x00` |
| 7 | ADDITIONAL CDB LENGTH | `0x18` |
| 8 | SERVICE ACTION [15:8] | `0x1F` |
| 9 | SERVICE ACTION [7:0] | `0xF0` |
| 10 | PROTOCOL / EXTEND | `(PROTOCOL << 1) \| EXTEND` |
| 11 | 転送フラグ | [共通ビット配置](#sat-transfer-flags) |
| 12〜13 | Reserved | 全て `0x00` |
| 14 | LBA [47:40] | 指定値 |
| 15 | LBA [39:32] | 指定値 |
| 16 | LBA [31:24] | 指定値 |
| 17 | LBA [23:16] | 指定値 |
| 18 | LBA [15:8] | 指定値 |
| 19 | LBA [7:0] | 指定値 |
| 20 | FEATURES [15:8] | 指定値 |
| 21 | FEATURES [7:0] | 指定値 |
| 22 | COUNT [15:8] | 指定値 |
| 23 | COUNT [7:0] | 指定値 |
| 24 | DEVICE | 指定値 |
| 25 | COMMAND | ATA opcode |
| 26 | Reserved | `0x00` |
| 27 | ICC [7:0] | 指定値 |
| 28 | AUXILIARY [31:24] | 指定値 |
| 29 | AUXILIARY [23:16] | 指定値 |
| 30 | AUXILIARY [15:8] | 指定値 |
| 31 | AUXILIARY [7:0] | 指定値 |

EXTEND および共通フィールドは [§4](#sat-cdb16)、[§3](#sat-cdb12) を参照。

<a id="sat-translations"></a>

## 6 ATA PASS-THROUGH CDB field translations

28bit形式は (12)、または EXTEND=0 の (16)/(32)。
48bit形式は EXTEND=1 の (16)/(32) に適用し、(12) には適用しない。

| CDB field | 48bit ATA field | 28bit ATA field |
| --- | --- | --- |
| FEATURES [15:8] | FEATURE [15:8] | n/a |
| FEATURES [7:0] | FEATURE [7:0] | FEATURE [7:0] |
| COUNT [15:8] | COUNT [15:8] | n/a |
| COUNT [7:0] | COUNT [7:0] | COUNT [7:0] |
| LBA [47:24] | LBA [47:24] | n/a |
| LBA [23:0] | LBA [23:0] | LBA [23:0] |
| DEVICE [7:4] | DEVICE [7:4] | DEVICE [7:4] |
| DEVICE [3:0] | DEVICE [3:0] | LBA [27:24] |
| COMMAND | COMMAND | COMMAND |
| AUXILIARY [31:0] | AUXILIARY [31:0] | n/a |
| ICC | ICC | n/a |

AUXILIARY/ICC を持つのは (32) のみ。(16) から対応する48bit ATAフィールドへ渡す値は0。

<a id="sat-transfer-width"></a>

### 6.1 EXTEND と転送長の参照幅

| T_LENGTH | EXTEND=0 | EXTEND=1 |
| --- | --- | --- |
| `00` | 無転送 | 無転送 |
| `01` | FEATURES [7:0] | FEATURES [15:0] |
| `10` | SECTOR_COUNT [7:0] | SECTOR_COUNT [15:0] |
| `11` | TPSIU | TPSIU |

転送長は符号なし整数。SECTOR_COUNT は CDB の COUNT に対応する。

<a id="sat-status"></a>

## 7 ATA PASS-THROUGH status return

ERROR / DEVICE FAULT は ATA STATUS のビット。

| CK_COND | ERROR | DEVICE FAULT | 結果 |
| --- | --- | --- | --- |
| `0` | `0` | `0` | GOOD |
| `1` | `0` | `0` | CHECK CONDITION、RECOVERED ERROR / ATA PASS-THROUGH INFORMATION AVAILABLE |
| 任意 | 任意 | `1` | CHECK CONDITION、エラー情報を返す |
| 任意 | `1` | `0` | CHECK CONDITION、エラー情報を返す |

CHECK CONDITION時のdescriptor形式senseには ATA Status Return descriptor を含める。
CK_COND=1 で PIO Data-In または NCQ の場合、descriptor形式ではbyte 3〜13を0にし、
固定形式では INFORMATION と COMMAND-SPECIFIC INFORMATION を0にする。

<a id="sat-descriptor"></a>

## 8 ATA Status Return sense data descriptor

DESCRIPTOR CODE=`0x09`、ADDITIONAL DESCRIPTOR LENGTH=`0x0C`。
ERROR、COUNT、LBA、DEVICE、STATUS を返す。
(12) または EXTEND=0 の (16)/(32) は28bit形式、EXTEND=1 の (16)/(32) は48bit形式。

| Bytes | Field name | Value |
| ---: | --- | --- |
| 0 | DESCRIPTOR CODE | `0x09` |
| 1 | ADDITIONAL DESCRIPTOR LENGTH | `0x0C` |
| 2 | Reserved / EXTEND | bit 7:1は0、bit 0はEXTEND |
| 3 | ERROR | ATA ERROR |
| 4 | COUNT [15:8] | EXTEND=0では0 |
| 5 | COUNT [7:0] | ATA COUNT [7:0] |
| 6 | LBA [31:24] | EXTEND=0ではbit 7:4を0にする |
| 7 | LBA [7:0] | ATA LBA [7:0] |
| 8 | LBA [39:32] | EXTEND=0では0 |
| 9 | LBA [15:8] | ATA LBA [15:8] |
| 10 | LBA [47:40] | EXTEND=0では0 |
| 11 | LBA [23:16] | ATA LBA [23:16] |
| 12 | DEVICE | ATA DEVICE |
| 13 | STATUS | ATA STATUS |

<a id="sat-fixed-sense"></a>

## 9 Fixed format sense data

以下のBytesは各フィールド内の位置。VALIDは規定されていない。

| Bytes | Field name | Value |
| ---: | --- | --- |
| 0 | INFORMATION: ERROR | ATA ERROR |
| 1 | INFORMATION: STATUS | ATA STATUS |
| 2 | INFORMATION: DEVICE | ATA DEVICE |
| 3 | INFORMATION: COUNT [7:0] | ATA COUNT [7:0] |
| 0 | COMMAND-SPECIFIC INFORMATION: flags | bit 7=EXTEND、6=COUNT UPPER NONZERO、5=LBA UPPER NONZERO、4=Reserved、3:0=LOG INDEX |
| 1 | COMMAND-SPECIFIC INFORMATION: LBA [7:0] | ATA LBA [7:0] |
| 2 | COMMAND-SPECIFIC INFORMATION: LBA [15:8] | ATA LBA [15:8] |
| 3 | COMMAND-SPECIFIC INFORMATION: LBA [23:16] | ATA LBA [23:16] |

EXTEND=1ではCOUNT [15:8]とLBA [47:24]を返せない。
COUNT UPPER NONZEROとLBA UPPER NONZEROは、それぞれ返せない上位値が0以外かを示す。
LOG INDEXが0以外ならdescriptor形式のsenseがATA PASS-THROUGH Results logに保存されており、
対応するPARAMETER CODEはLOG INDEX−1。
