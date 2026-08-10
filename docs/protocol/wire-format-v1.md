# QRBeam v1 线协议

## 1. 文档状态

- **协议版本：** `1`
- **字节序：** 小端（Little-endian）
- **帧头长度：** `56 B`
- **最大二维码帧：** `2,953 B`
- **原子编码符号：** `256 B`
- **默认文件区块：** `512 KiB`
- **参考实现：** `crates/qrbeam-core`

本文定义 QRBeam 协议核心第一阶段的稳定字节格式。Rust 黄金测试已经锁定帧和清单编码；WASM、iOS 和 Android 尚未接入，因此当前格式不能视为已完成跨端兼容验证。

## 2. 通用规则

1. 所有多字节整数使用小端编码。
2. 未知协议版本、未知枚举值和非零保留字段必须拒绝。
3. 解析器必须先验证长度，再读取可变字段或分配内存。
4. 帧 CRC32C 失败时不得更新会话、区块图或符号去重状态。
5. 文件只有在区块 CRC32C 和最终 BLAKE3 全部通过后才算完成。

## 3. QR 帧

### 3.1 固定帧头

每个 QR 帧由 `56 B` 固定帧头和可变 payload 组成。

| 偏移 | 长度 | 字段 | 类型 | 说明 |
|---:|---:|---|---|---|
| `0` | 4 | `magic` | `[u8; 4]` | 固定 ASCII `QRBM` |
| `4` | 1 | `protocol_version` | `u8` | 固定为 `1` |
| `5` | 1 | `frame_type` | `u8` | 见帧类型表 |
| `6` | 1 | `flags` | `u8` | 清单末片使用 bit 0；其他位保留 |
| `7` | 1 | `channel_id` | `u8` | 同一显示时刻的二维码通道号 |
| `8` | 1 | `profile_id` | `u8` | 当前帧使用的带宽档位 |
| `9` | 1 | `reserved` | `u8` | 必须为 `0` |
| `10` | 2 | `header_length` | `u16` | 固定为 `56` |
| `12` | 16 | `session_id` | `[u8; 16]` | 128 位文件会话 ID |
| `28` | 4 | `file_id` | `u32` | 当前会话内的文件 ID |
| `32` | 8 | `global_frame_index` | `u64` | 用户可定位的显示时间轴地址 |
| `40` | 4 | `segment_index` | `u32` | 数据区块号；清单帧中为分片号 |
| `44` | 4 | `first_symbol_id` | `u32` | 首个 RaptorQ ESI；清单帧中为分片总数 |
| `48` | 2 | `symbol_count` | `u16` | payload 中连续符号数量 |
| `50` | 2 | `payload_length` | `u16` | payload 精确字节数 |
| `52` | 4 | `crc32c` | `u32` | 完整帧 CRC32C |
| `56` | 可变 | `payload` | bytes | 最大 `2,897 B` |

### 3.2 帧类型

| 值 | 名称 | 用途 |
|---:|---|---|
| `1` | `Manifest` | 文件清单分片 |
| `2` | `Test` | 接收能力测试 |
| `3` | `Data` | RaptorQ 系统符号 |
| `4` | `Repair` | RaptorQ 修复符号 |
| `5` | `Control` | 保留控制帧 |

其他值必须返回未知帧类型错误。

### 3.3 CRC32C

CRC32C 覆盖完整帧头和 payload。计算时把偏移 `52..56` 的 CRC 字段临时视为 4 个零字节：

```text
crc32c(
  header[0..52]
  || 00 00 00 00
  || payload
)
```

计算结果以 `u32` 小端写入偏移 `52..56`。

### 3.4 数据和修复 payload

`Data` 与 `Repair` 帧满足：

```text
payload_length = symbol_count × 256
```

payload 按顺序拼接连续 ESI：

```text
symbol(first_symbol_id)
|| symbol(first_symbol_id + 1)
|| symbol(first_symbol_id + symbol_count - 1)
```

单帧不得跨越文件区块，也不得跨越系统符号和修复符号的边界。

## 4. 文件清单

### 4.1 固定部分

完整清单由 `112 B` 固定部分、可变字段和末尾 `4 B` CRC32C 组成。

| 偏移 | 长度 | 字段 | 类型 | 说明 |
|---:|---:|---|---|---|
| `0` | 4 | `magic` | `[u8; 4]` | 固定 ASCII `QRMF` |
| `4` | 1 | `protocol_version` | `u8` | 固定为 `1` |
| `5` | 1 | `compression` | `u8` | `0 = none`，`1 = gzip` |
| `6` | 2 | `reserved` | `u16` | 必须为 `0` |
| `8` | 16 | `session_id` | `[u8; 16]` | 必须与外层清单帧一致 |
| `24` | 4 | `file_id` | `u32` | 必须与外层清单帧一致 |
| `28` | 8 | `original_length` | `u64` | 原始文件长度，最大 `100,000,000 B` |
| `36` | 8 | `container_length` | `u64` | 光学传输容器长度 |
| `44` | 32 | `file_hash` | `[u8; 32]` | 原始文件 BLAKE3 |
| `76` | 4 | `segment_size` | `u32` | v1 固定为 `524,288` |
| `80` | 2 | `symbol_size` | `u16` | v1 固定为 `256` |
| `82` | 1 | `profile_count` | `u8` | 后续档位记录数量 |
| `83` | 1 | `reserved` | `u8` | 必须为 `0` |
| `84` | 4 | `last_segment_length` | `u32` | 最后区块实际长度 |
| `88` | 16 | `encoding_seed` | `[u8; 16]` | 确定性发送种子 |
| `104` | 2 | `filename_length` | `u16` | UTF-8 文件名字节数，最大 `255 B` |
| `106` | 2 | `mime_length` | `u16` | UTF-8 MIME 字节数，最大 `255 B` |
| `108` | 4 | `segment_count` | `u32` | 区块 CRC32C 数量 |

### 4.2 可变部分

偏移 `112` 后面的字段严格按以下顺序排列：

```text
filename[filename_length]
mime_type[mime_length]
profiles[profile_count × 8]
segment_crc32c[segment_count × 4]
manifest_crc32c[4]
```

清单 CRC32C 覆盖 CRC 字段之前的全部清单字节。CRC 本身以 `u32` 小端追加到末尾。

### 4.3 档位记录

每个档位固定占 `8 B`：

| 相对偏移 | 长度 | 字段 | 说明 |
|---:|---:|---|---|
| `0` | 1 | `profile_id` | 档位 ID |
| `1` | 1 | `symbols_per_frame` | 每帧原子符号数，范围 `1..=11` |
| `2` | 1 | `ecc` | `0 = L`，`1 = M` |
| `3` | 1 | `target_fps` | 范围 `1..=60` |
| `4` | 1 | `min_module_pixels` | 最小物理模块像素数 |
| `5` | 1 | `max_qr_version` | 范围 `1..=40` |
| `6` | 2 | `reserved` | 必须为 `0` |

v1 默认档位：

| ID | 符号数 | ECC | FPS | 最小模块像素 | 最大 QR 版本 |
|---:|---:|---|---:|---:|---:|
| `0` | 1 | M | 8 | 6 | 40 |
| `1` | 5 | L | 24 | 6 | 40 |
| `2` | 8 | L | 30 | 4 | 40 |
| `3` | 11 | L | 60 | 4 | 40 |

高速档每个数据帧的固定长度为 `56 + 11 × 256 = 2,872 B`，小于二维码帧上限 `2,953 B`。

## 5. 清单分片

完整清单可能超过稳定通道的 `256 B` payload。发送端按以下方式分片：

1. 对包含末尾 CRC32C 的完整清单计算 BLAKE3。
2. 把完整清单切成最多 `224 B` 的连续分片。
3. 每个清单帧 payload 写入 `32 B` 完整清单 BLAKE3，再写入本片数据。
4. 帧头 `segment_index` 保存分片号，从 `0` 开始。
5. 帧头 `first_symbol_id` 保存分片总数。
6. 最后一片把 `flags` 的 bit 0 设为 `1`。

接收端可以乱序和重复接收分片。只有在分片全部收齐、完整清单 BLAKE3、清单 CRC32C、外层 `session_id` 和 `file_id` 全部一致后，才能创建接收会话。

## 6. 分段 RaptorQ

每个文件区块是独立 RaptorQ 对象。v1 固定使用：

```rust
ObjectTransmissionInformation::new(
    actual_segment_length,
    256,
    1,
    1,
    1,
)
```

- `source_block_number` 固定为 `0`。
- QRBeam `segment_index` 标识文件区块。
- 区块源符号数量 `K = ceil(actual_segment_length / 256)`。
- 系统符号 ESI 范围为 `0..K`。
- 首个修复符号 ESI 为 `K`，后续修复 ESI 单调增加。
- ESI 是 24 位无符号值，必须小于 `16,777,216`。
- 最后一个不足 `256 B` 的源符号由 RaptorQ 补零；恢复后截断到清单中的实际区块长度。

区块恢复后先验证清单中的区块 CRC32C。全部区块完成后按 `segment_index` 拼接，验证精确容器长度和最终文件 BLAKE3。

协议核心第一阶段只生成 `compression = none` 的发送会话。`gzip` 已保留线格式枚举，流式压缩和解压由后续存储计划实现。

## 7. 会话和时间轴

- 同一显示时刻的所有通道共享 `global_frame_index`，使用不同 `channel_id`。
- 每个通道必须分配不同的 ESI 范围。
- 历史定位重新播放原来的 `FramePlan`，因此可以确定性重建相同帧。
- 调整 profile、FPS、布局或通道数不改变 `session_id` 和 `file_id`。
- 文件、协议版本、区块参数或编码种子变化时必须创建新会话。

## 8. 黄金向量

### 8.1 帧向量

输入：

- `frame_type = Data`
- `channel_id = 2`
- `profile_id = 3`
- `session_id = 11` 重复 16 次
- `file_id = 0x01020304`
- `global_frame_index = 0x0102030405060708`
- `segment_index = 9`
- `first_symbol_id = 10`
- `symbol_count = 1`
- payload 为 `A5` 重复 256 次

结果：

- 总长度：`312 B`
- CRC32C：`0x42703834`
- 完整帧 BLAKE3：`54d2ce88fd87877efbde6036a274b68f2fd8ceff4cbc13b9a888c33c1e550c73`
- 56 字节帧头：

```text
5152424d0103000203003800
11111111111111111111111111111111
04030201
0807060504030201
09000000
0a000000
0100
0001
34387042
```

帧头后紧跟 `A5` 重复 256 次。

### 8.2 清单向量

输入字段与 `tests/manifest_wire.rs` 的 `sample_manifest()` 相同。编码结果：

- 总长度：`191 B`
- CRC32C：`0xae26d16e`
- 完整清单 BLAKE3：`221b392a651153a455a08eca071ff61258235157b2bdc8dfec13d46dd4a34c94`

```text
51524d4601000000
22222222222222222222222222222222
07000000
40420f0000000000
40420f0000000000
3333333333333333333333333333333333333333333333333333333333333333
000008000001040040420700
44444444444444444444444444444444
0b00180002000000
6578616d706c652e62696e
6170706c69636174696f6e2f6f637465742d73747265616d
0001010806280000
0105001806280000
0208001e04280000
030b003c04280000
78563412efcdab90
6ed126ae
```

黄金测试直接比较这些固定字节，不以本库 encode/decode 往返代替独立期望值。

## 9. 解析拒绝条件

接收端至少拒绝以下输入：

- 帧短于 `56 B`；
- magic、版本、帧头长度或保留字段错误；
- 声明 payload 长度与实际长度不同；
- 完整帧超过 `2,953 B`；
- 数据或修复 payload 不是 `symbol_count × 256 B`；
- ESI 越过 24 位上限；
- CRC32C 或 BLAKE3 不匹配；
- 清单字符串不是 UTF-8；
- 清单区块数量、最后区块长度和容器长度不一致；
- 清单分片哈希、总数、会话或文件 ID 冲突；
- 数据帧跨入修复 ESI，或修复帧从系统 ESI 开始；
- 帧来自其他会话或文件；
- 最终容器长度或文件 BLAKE3 不一致。
