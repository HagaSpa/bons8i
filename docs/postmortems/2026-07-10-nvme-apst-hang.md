# Postmortem: NVMe SSD が4日間 PCIe バス上から消失（APST ハング）

- **日付**: 2026-07-10（発見・対処・復旧同日）
- **環境**: Raspberry Pi 5 8GB / Ubuntu 24.04 arm64 / kubeadm シングルノードクラスタ / Waveshare PoE M.2 HAT+ (B) / KIOXIA BG4 256GB (KBG40ZNS256G)
- **影響**: 実害なし（SSD は未マウント・未使用だったため）。ただし**発生から発見まで4日間、検知できなかった**
- **ステータス**: 解決済み（恒久対処適用・再発監視中）

## 事象

監視スタック導入に向けた PVC 配置の検討中、`lsblk` で **NVMe が `0B` 表示**であることを発見:

```
nvme0n1         0B disk
```

KIOXIA BG4 256GB なら 238.5G と表示されるはず。`/dev/nvme0`・`/dev/nvme0n1` はデバイスファイルとして存在 = カーネルはデバイスの存在を認識しているが、容量を読めていない状態。

### 前史: ACT LED の点滅（当初は別事象と考えられていた）

約1週間前から、PoE M.2 HAT 上の「ACT」LED が点滅を継続していた。当初は「Kubernetes がクラスタ状態を microSD に常時書き込んでいるため」と推定して放置。**この推定は後に誤りと判明**（後述の LED 同定）。

## タイムライン

| 日時 | 出来事 |
|---|---|
| ~07-03 | HAT 上の ACT LED の連続点滅に気づく。「k8s の SD 書き込み」と推定して放置 |
| 07-04 10:32 | 再起動（別作業）。NVMe は正常認識（後の dmesg 調査で確認） |
| 07-06 夕方 | **NVMe コントローラがバスから消失**（起動から 199877 秒 ≒ 2.31 日後）。無症状のまま経過 |
| 07-10 夜 | `lsblk` で 0B を発見 → dmesg で根本原因特定 → カーネルパラメータ追加 + 再起動 → **238.5G で復活** |
| 同日 | SMART による健康診断・LED の同定実験を実施し、前史の誤推定を訂正 |

## 調査

### dmesg（カーネルの言い分を聞く）

```
sudo dmesg | grep -iE "nvme|pcie" | tail -30
```

起動時（0.6〜16秒）は完全に正常:

```
nvme nvme0: pci function 0000:01:00.0
nvme nvme0: allocated 61 MiB host memory buffer.   ← BG4 は DRAM レス。ホスト RAM を借りる設計
nvme nvme0: 4/0/0 default/read/poll queues          ← I/O キュー確立
block nvme0n1: No UUID available providing old NGUID
```

起動から 199877 秒後に突然死。**カーネルが原因候補と対処パラメータまで名指し**していた:

```
nvme nvme0: controller is down; will reset: CSTS=0xffffffff, PCI_STATUS=0xffff
nvme nvme0: Does your device have a faulty power saving mode enabled?
nvme nvme0: Try "nvme_core.default_ps_max_latency_us=0 pcie_aspm=off pcie_port_pm=off" and report a bug
nvme 0000:01:00.0: Unable to change power state from D3cold to D0, device inaccessible
nvme nvme0: Disabling device after reset failure: -19
```

読み解き:

- `CSTS=0xffffffff, PCI_STATUS=0xffff` — 全ビット1は**レジスタの値ではなく「バスから応答が無いときに読める値」**。デバイスが PCIe バス上から実質消えた
- `D3cold to D0` — PCIe の電源状態遷移。D3cold = 最深の省電力、D0 = 通常動作。「深い眠りから起こしても起きてこない」
- `-19` (ENODEV) — リセット失敗によりカーネルがデバイスを切り離した

## 根本原因

**NVMe APST（Autonomous Power State Transition = 自律的省電力遷移）と Raspberry Pi 5 の相性問題。**

NVMe には「アイドルが続いたら自分で深い省電力状態へ落ちる」APST 機能がある。一部のドライブ（特に DRAM レス機）と特定プラットフォーム（Pi 5 は報告例が多い）の組み合わせで、**深い省電力状態から復帰できなくなる既知の相性問題**が存在する。

本件はその典型的な発症条件を満たしていた:

- SSD は**未マウントで完全アイドル**（ホスト I/O が一切ない）
- 約 2.3 日間の完全アイドルで最深状態（PS4, 5mW）へ遷移し、復帰不能に

ドライブの Supported Power States 表（`smartctl -a`）に状況証拠が写っている:

```
St Op   Max      ...  Ent_Lat  Ex_Lat
 3 -    0.0500W         800    1200
 4 -    0.0050W        3000   32000   ← 非動作状態（Op=−）。復帰 32ms と自己申告するが、実際には戻れなかった
```

なお「中古品由来の故障」ではなく、**相性 + 完全アイドルという条件が揃って初めて発症**したもの（後述の SMART 結果はほぼ新品相当）。

## 対処

`/boot/firmware/cmdline.txt`（1行ファイル）の行末に以下を追記して再起動:

```
nvme_core.default_ps_max_latency_us=0
```

意味: 「復帰レイテンシ 0μs を超える省電力状態を使うな」= APST の深い状態（PS3/PS4）を全面禁止。トレードオフはアイドル消費電力が数十 mW → ~2.2W へ上がることのみ（PoE 給電の予算内で許容）。

**結果: 再起動後 `nvme0n1 238.5G` で完全復活。** dmesg の起動シーケンスも正常。

カーネルが提示した残り2パラメータ（`pcie_aspm=off pcie_port_pm=off` = より広範囲な PCIe 省電力の停止）は**適用せず**、まず最小の1つで様子を見る段階的アプローチを選択。

## 検証

### SMART 健康診断（`smartctl -a /dev/nvme0`）

| 項目 | 値 | 読み |
|---|---|---|
| Power On Hours | 648h | 通電歴1ヶ月弱 |
| Data Units Written | 80.8 GB | 256GB に対し累計 0.3 回分 |
| Percentage Used | 0% | 寿命消費ゼロ |
| Media/Data Integrity Errors | 0 | メディア障害なし |
| 総合 | **PASSED** | |

補足: Unsafe Shutdowns 12（前歴・実害なし）、アイドル 53°C（警告閾値 82°C に対し余裕はあるが、監視導入後の観察対象とする）。

### LED の同定実験（前史の誤推定の訂正）

「復旧後も HAT の ACT LED が消灯している」ことが当初の推定（k8s 書き込み説）と矛盾したため、切り分けを実施:

| 実験 | 結果 |
|---|---|
| `cat /sys/class/leds/ACT/trigger` | `[mmc0]` = カーネル管理の ACT は SD カード活動に連動 |
| SD へ 200MB 直書き（`dd oflag=direct`） | **HAT の ACT は光らない** |
| `echo default-on > trigger` | **Pi 基板上（microSD 横）の緑 LED** が反応（active-low 配線） |
| NVMe から 1GB 読み出し（`dd`） | **HAT の ACT が点滅** → NVMe activity LED と確定 |
| `/proc/diskstats` を 10 秒間隔で2回取得 | カウンタ完全一致 = ホスト I/O ゼロでも点滅（ファームウェアの自律動作） |

確定した対応関係:

| LED | 正体 |
|---|---|
| HAT 上「ACT」 | **NVMe activity LED**（SSD 自身が M.2 ピン経由で駆動。カーネル管理外） |
| Pi 基板上 microSD 横の緑 | **カーネル管理の ACT**（`[mmc0]`・active-low）。k8s の SD 書き込みはこちらに出ていた |

1週間前からの点滅の正体は **NVMe ファームウェアの自律動作**であり、当初の推定（k8s の SD 書き込み）は LED の同定を誤っていた。

## 何がうまくいき、何がうまくいかなかったか

**うまくいったこと**

- dmesg がほぼ答えを出していた。「カーネルログを最初に読む」が最短経路だった
- 対処を最小の1パラメータに絞り、エスカレーションパス（`pcie_aspm=off` 等）を残した

**うまくいかなかったこと**

- **検知の失敗**: デバイス消失から発見まで4日。監視・アラートが存在しなかった
- **誤った初期仮説の放置**: LED 点滅に「もっともらしい説明」が付いた時点で検証せず納得してしまった。トリガー設定の確認と負荷生成という 60 秒の実験で同定できたにもかかわらず、1週間放置した

この障害は、まさに導入準備中だった監視スタック（VictoriaMetrics + Alertmanager → GitHub issue 自動起票）の必要性をそのまま実証する形になった。

## フォローアップ

- [ ] NVMe の用途決定: (a) データディスク化（fstab マウント + local-path-provisioner の保存先変更）or (b) ブート移行（EEPROM 変更込み）
- [ ] 再発監視: dmesg に nvme エラーが出ないか数日間観察。再発時は `pcie_aspm=off pcie_port_pm=off` を追加
- [ ] 監視導入後: NVMe 温度（ベースライン 53°C）と PCIe/NVMe エラーをダッシュボード・アラート対象に含める
