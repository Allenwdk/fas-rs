# 小米 FEAS 帧调度内核模块深度分析报告

> 分析范围：`xiaomifeas/` 目录全部 7 个源文件，共约 3200 行 C 代码
> 分析时间：2026-09-01
> 分析方法：5 子系统并行深度分析 + 完备性交叉评审

---

## 目录

1. [架构总览](#1-架构总览)
2. [CPU 帧调度策略 (perfmgr_policy.c)](#2-cpu-帧调度策略-perfmgr_policyc)
3. [GPU 帧调度 (perfmgr_gpu.c)](#3-gpu-帧调度-perfmgr_gpuc)
4. [DDR/L3 联动 (perfmgr_remap.c)](#4-ddrl3-联动-perfmgr_remapc)
5. [DCVS 仲裁 (dcvs_arbi/dcvs_main.c)](#5-dcvs-仲裁-dcvs_arbidcvs_mainc)
6. [入口链路与触摸 (perfmgr_main/perfmgr_ioctl/speed_touch)](#6-入口链路与触摸)
7. [跨模块交互与数据流](#7-跨模块交互与数据流)
8. [完整参数表](#8-完整参数表)
9. [遗留风险与调优建议](#9-遗留风险与调优建议)

---

## 1. 架构总览

### 1.1 整体架构 ASCII 数据流

```
用户态 HWUI / SurfaceFlinger
   │
   ├─ ioctl BUFFER_QUEUE           (msg.start=1: EGL_QUEUE, 2: SF_COMPLETION; start=0 丢弃)
   ├─ ioctl BUFFER_GPU_HINT        (msg.start/msg.pid/msg.frame_id → GPU 帧起止)
   └─ sysfs speed_touch/sf_available_buffer_size (触摸 producer/consumer 提频)
   ▼
/proc/perfmgr/perf_ioctl  (perfmgr_ioctl.c device_ioctl)
   ├─ perfmgr_notify_qudeq_fp    = perfmgr_notify_qudeq        (BUFFER_QUEUE)
   ├─ perfmgr_notify_gpu_hint_fp = perfmgr_notify_gpu_hint     (BUFFER_GPU_HINT)
   └─ perfmgr_notify_connect_fp  = NULL (从未赋值 → 死路径)
   ▼
perfmgr_notify_qudeq (perfmgr_main.c)
   ├─ perfmgr_is_enable()：15s 延迟启用状态机
   │   disable → enable_pending (15s 计时) → enabled
   ├─ 分配 PERFMGR_NOTIFIER_PUSH_TAG (但用了 sizeof(connected_buffer))
   └─ ordered wq → perfmgr_notify_qudeq_cb
                    ├─ perfmgr_notify_connect_cb：connected_buffer_list 节点管理
                    └─ hint==EGL_QUEUE      → perfmgr_do_policy (CPU 频率决策)
                       hint==SF_COMPLETION  → perfmgr_update_sf_msg (TODO, 仅 printk)
   ▼
perfmgr_do_policy (perfmgr_policy.c)  ←── 核心 CPU 调频决策
   ├─ frame_usecs64 = 帧间隔(μs); last_frame_unit[5] 滑动窗口
   ├─ calulate_fps：目标 fps 检测 (144/120/121/90/91/60/61/45/49/30)
   ├─ frame_usecs64_x_fps / frame_unit_usecs64_x_fps (×fps 归一化: 理想值=1e6)
   ├─ 三级判定：jank(level 1-5) / predict(level 1-3) / keep-down(level -1/-2)
   └─ perfmgr_set_freq
        ├─ better_perf=1 → perfmgr_set_ceiling_and_floor + hrtimer 三级释放
        │     LIMIT(0x02) --timeout_fps→ FLOOR(0x03) --timeout_left→
        │     FLOOR_HIGH(0x04) --timeout_left→ RELEASED_ALL(0x01)
        └─ better_perf=0 → do_frame_limit_freq (仅设 max)
              │
              ▼
        update_policy_online → cpufreq_adjust_notify
              ├─ freq_qos_update_request(MAX/MIN) 每 CPU
              └─ perfmgr_boost_dcvsfreq_and_timeout(ib_max)
                    │
                    ▼
              perfmgr_remap.c：d_boost/l3freq_boost → 查 remap 表 → boost_dcvs_freq
                    └─ delayed_work 1500ms 释放
   ▼
perfmgr_notify_gpu_hint (perfmgr_gpu.c)
   ├─ 帧 START → 记录 start_time, calculate_rescue_time() → hrtimer 预提频
   ├─ 帧 END(同帧) → 结算 GPU 耗时, 5 帧窗口 avg
   │     ├─ 快降(release): comp<=阈值 → cur_level++(低频) → boost
   │     ├─ 取消救援: comp<=rescue_budget → hrtimer_cancel
   │     └─ 超时: 仅日志
   └─ perfmgr_boost_gpu → target_gpu_freq + delayed_work 1500ms 释放
   ▼
kgsl/devfreq governor → perfmgr_get_gpu_hook (EXPORT_SYMBOL)
   └─ 覆盖 *freq = target_gpu_freq

┌─ dcvs_arbi (独立表决库, 无自采样源, 外部 memlat 驱动回调) ──────────────┐
│  mi_record_pre_sampling_stats → mi_calculate_sampling_stats              │
│  → mi_calculate_mon_sampling_freq(采样档) / mi_update_memlat_fp_vote     │
│     (FP 地板 600M↔1500M 迟滞) / mi_cpufreq_to_memfreq                   │
│  stat_data() → 外部 AOSS/dcvs 硬件                                        │
│  与 remap 同域并存但不互调                                                  │
└──────────────────────────────────────────────────────────────────────────┘
```

### 1.2 模块文件清单

| 文件 | 行数 | 职责 |
|------|------|------|
| `perfmgr/perfmgr.h` | 152 | 公共头文件：结构体、枚举、ioctl 命令、函数声明 |
| `perfmgr/perfmgr_main.c` | 425 | 入口：buffer 管理、延迟启用、filter 过滤、workqueue 调度 |
| `perfmgr/perfmgr_policy.c` | 899 | **CPU 调频策略核心**：帧率识别、jank 救援、预测提频、hrtimer 状态机 |
| `perfmgr/perfmgr_gpu.c` | 647 | **GPU IFFM**：帧起止 hook、5 帧窗口、rescue/release、get_gpu_hook |
| `perfmgr/perfmgr_ioctl.c` | 136 | procfs `perf_ioctl` 入口，ioctl 命令分发 |
| `perfmgr/perfmgr_remap.c` | 233 | CPU→DDR/L3 频率映射 + 超时释放 |
| `input/speed_touch.c` | 170 | 触摸输入 producer/consumer 提频切换 |
| `dcvs_arbi/dcvs_main.c` | 524 | DCVS 仲裁：memlat 采样统计分析、FP 投票 |
| 合计 | ~3183 | |

---

## 2. CPU 帧调度策略 (perfmgr_policy.c)

### 2.1 算法流程

**触发链**：用户态 HWUI/SurfaceFlinger 每帧 buffer queue → `perfmgr_notify_qudeq()` → workqueue → `perfmgr_notify_qudeq_cb()` → `perfmgr_do_policy(priv)`。

**帧率识别** (`calulate_fps`)：初始 `target_fps==0` 时用最近 5 帧滑动平均，之后累加 30 帧总耗时后取平均映射到离散 FPS。帧时间阈值(μs)：

| 帧时间范围 (μs) | 判定 fps |
|---|---|
| `< 8130` | 144 |
| `< 10753` | 120 或 121 |
| `< 16129` | 90 或 91 |
| `< 20000~21277` | 60 或 61 |
| `< 31250` | 45 或 49 |
| `< 50000` | 30 |
| `≥ 50000` | -1 (未知) |

**核心判定公式**：`frame_usecs64 × target_fps`。物理意义：帧时长(μs/frame) × 帧率(frame/s) = μs/s。理想帧恰为 1,000,000。

**三级判定逻辑**：

1. **Jank 救援分支**：`frame_usecs64_x_fps > 1,000,000 + scaling_r_thres×1000`（默认 1,700,000 ≈ 170% 帧时长）
   - `r_perf=0`：按 `r_step` 分 2 级
   - `r_perf=1`：按 `1~5×r_step` 分 5 级
   - 效果：`set_freq_level = last_freq_level - r_freq_level - level`

2. **预测提频分支**：`frame_unit_usecs64_x_fps > 5,000,000 + scaling_a×1000`（默认 5,380,000 ≈ 107.6% 单帧）
   - 5 帧滑动窗口显示帧变慢，提频防止卡顿

3. **保持/降频分支**：`frame_unit_usecs64_x_fps < 5,000,000 - scaling_b×1000`（默认 5,050,000 ≈ 101% 单帧）
   - 累计 `rescue_keep_count≥total` 或 `keep_continus_count>cons_no_j_cnt` 且冷却足够 → 降频

### 2.2 hrtimer 三级状态机

```
状态 0: PERFMGR_FREQUENCY_LIMIT_STATE (0x02) — 主动提频/限频中
  定时器到期: last_freq_level -= timeout_r_freq_level
  → 迁至 PERFMGR_FREQUENCY_RELEASE_AND_SET_FLOOR (0x03)
  → queue_work(do_current_work): 设地板 f_left_minfreq
  → hrtimer_forward_now(timeout_left=2333ms) → HRTIMER_RESTART

状态 1: PERFMGR_FREQUENCY_RELEASE_AND_SET_FLOOR (0x03) — 释放天花板, 保持地板
  定时器到期: 迁至 PERFMGR_FREQUENCY_RELEASE_AND_SET_FLOOR_HIGH (0x04)
  → queue_work(do_current_work): 设地板
  → hrtimer_forward_now(timeout_left=2333ms) → HRTIMER_RESTART

状态 2: PERFMGR_FREQUENCY_RELEASE_AND_SET_FLOOR_HIGH (0x04) — 准备完全释放
  定时器到期: 迁至 PERFMGR_FREQUENCY_RELEASED_ALL_STATE (0x01)
  → queue_work(do_current_work): do_maxfreq_release()
  → HRTIMER_NORESTART (终止)
```

此外还有一个 **50ms delayed_work 安全网**：每帧 `cancel_delayed_work(&release_work)` 后 `maxfreq_release_timeout(50000μs)`。

### 2.3 发现的问题

| 严重度 | 问题 | 证据 |
|--------|------|------|
| **高** | `update_policy_online` 错误路径 CPU hotplug 锁泄漏 | `perfmgr_policy.c:326-345` — `cpufreq_cpu_get` 返回 NULL 时直接 `return`，未 `put_online_cpus()` |
| **高** | `perfmgr_policy_init` 中 `cpufreq_cpu_get` 无对应 `cpufreq_cpu_put`，引用泄漏 | `perfmgr_policy.c:862-883` — 两处 get 无 put |
| **高** | 多 App 状态坍缩：`last_limit_time` 和 `down_count` 为 static 局部变量，所有渲染连接共享 | `perfmgr_policy.c:621-622` |
| **中** | 全局变量 `jank_happened` 和 `last_freq_level` 多连接共享竞态 | `perfmgr_policy.c:56-57` |
| **中** | `bus_get_dev_root` 返回的 device 引用未释放 | `perfmgr_policy.c:887-897` |
| **低** | `update_circle` 边界条件不对称（`>` 与 `>=` 不一致） | `perfmgr_policy.c:600-601` |
| **低** | `perfmgr_notify_qudeq_cb` 将负值错误码视为成功 | `perfmgr_main.c:289` |
| **低** | `perfmgr_alloc_atomic` 分配了比所需更大的内存 | `perfmgr_main.c:336` — `sizeof(connected_buffer)` 应为 `sizeof(PERFMGR_NOTIFIER_PUSH_TAG)` |

---

## 3. GPU 帧调度 (perfmgr_gpu.c)

### 3.1 算法流程

**触发链**：SurfaceFlinger/HWUI 每帧渲染通过 ioctl `BUFFER_GPU_HINT` 上报 start/pid/frame_id → `perfmgr_notify_gpu_hint(start, pid, frameNum)`。

**帧起止状态机**（以 `cur_frame_id` 区分同/异帧）：

- **帧 START**（start=1）：取消旧 hrtimer、记录 `start_time`、计算 `final_rescue_time` 并启动 hrtimer
- **帧 END**（start=0，同帧）：
  1. 计算 `frame_gpu_completion = ktime_sub(current_time, start_time)`（ns）
  2. 写入 5 帧滑动窗口得 avg
  3. 三路判定：
     - **快降**：单帧耗时 ≤ `release_time_60` 或 5 帧 avg ≤ `unit_release_time_60`（10.5ms）→ `cur_level++`（低频）
     - **取消救援**：comp ≤ `rescue_time_60`（16ms）→ `hrtimer_cancel`
     - **超时日志**：comp > `final_rescue_time` → 仅打印

**频率覆盖**：`perfmgr_get_gpu_hook` (EXPORT_SYMBOL) 被 kgsl governor 每次查询 target 频时调用，覆盖 `*freq = target_gpu_freq`。

### 3.2 发现的问题

| 严重度 | 问题 | 证据 |
|--------|------|------|
| **高** | **GPU 空指针崩溃**：`perfmgr_boost_gpu` 中 `perfmgr_df->dev.parent` 在 devfreq 未注册时解引用；`perfmgr_get_freq_level` 中 `gpu_freq_table` 同理。门控仅 `gpu_iffm_enable && is_freq_match`，不检查 devfreq 就绪 | `perfmgr_gpu.c:539, 206, 397` |
| **中** | **hrtimer 原子上下文调用可睡眠函数**：`gpufreq_boost_timer_func` 运行在硬中断上下文，调 `dev_pm_qos_update_request`（持 mutex）及外部回调 | `perfmgr_gpu.c:642-643, 630, 515-516, 577` |
| **中** | **救援逻辑死路**：`calculate_rescue_time` 预算耗尽时返回 0 → hrtimer 不启动、END 超时也不打日志，持续超预算时预提频永久失效 | `perfmgr_gpu.c:233-240, 361-363, 431-436` |
| **中** | **QoS 垫底自释放**：刚设的 `gpu_req_min_freq` 被同函数立即复位为 0，垫底逻辑形同虚设 | `perfmgr_gpu.c:546-552, 555-561` |
| **中** | **乱序帧 END 丢弃**：三级缓冲下 N 帧 END 晚于 N+1 帧 START 时仅 cancel timer 即 return，测量丢失 | `perfmgr_gpu.c:367-369` |
| **低** | 切换游戏后 5 帧窗口残留上一局数据（`last_frame_unit[5]` 未清零） | `perfmgr_gpu.c:90-91, 283-295` |
| **低** | `gpu_load_history` 环形数组从未被写入（死代码），proc 输出恒空 | `perfmgr_gpu.c:71, 133-148` |
| **低** | 全局变量无锁，ioctl/软中断/workqueue/hook 多上下文并发读写 | `perfmgr_gpu.c:38-39, 78-91, 318-440, 447-490, 596-609` |
| **低** | `cur_gpu_freq` 滞后（代码自注"更新不及时??"）导致提频档位偏差 | `perfmgr_gpu.c:454, 615` |

---

## 4. DDR/L3 联动 (perfmgr_remap.c)

### 4.1 算法

**触发链**：`cpufreq_adjust_notify` 检测到 `ib_max != policy->cpuinfo.max_freq` 时调用 `perfmgr_boost_dcvsfreq_and_timeout(ib_max)`。

**频率映射**：`perfmgr_boost_dcvsfreq_online(freq, hw_type)` 遍历 4 条 remap 条目，找到第一个满足 `freq > remap_table[i].cpufreq_khz` 的映射。默认表：

| DDR | CPU 频率 > | DDR 频率 |
|-----|-----------|----------|
| 0 | 1500000 kHz | 3187000 kHz |
| 1 | 1400000 kHz | 2736000 kHz |
| 2 | 1300000 kHz | 2092000 kHz |
| 3 | 1200000 kHz | 1708000 kHz |
| 兜底 | ≤1200000 | 547000 kHz |

| L3 | CPU 频率 > | L3 频率 |
|----|-----------|---------|
| 0 | 1700000 kHz | 1689600 kHz |
| 1 | 1500000 kHz | 1478400 kHz |
| 2 | 1300000 kHz | 1267200 kHz |
| 3 | 1200000 kHz | 1056000 kHz |
| 兜底 | ≤1200000 | 307200 kHz |

### 4.2 发现的问题

| 严重度 | 问题 | 证据 |
|--------|------|------|
| **高** | `perfmgr_ddrfreq_remap_set` 错误路径 `kstrdup` 后泄漏 `str`，DDR/L3 两处 | `perfmgr_remap.c:83, 89/92/98/100` |
| **中** | `perfmgr_boost_dcvsfreq_online` 未处理无效 `hw_type`，使用未初始化栈变量 | `perfmgr_remap.c:42-49, 54-66` |
| **中** | `d_table` / `l3freq_remap_table` 的 get 函数在 `cpufreq_khz==0` 时提前退出，读取不完整 | `perfmgr_remap.c:148-149` |
| **低** | `dcvsfreq_release_work` 无模块卸载清理（无 module_exit） | `perfmgr_remap.c:230-231` |
| **低** | `val = min(val, U32_MAX)` 对 u32 类型无效果，冗余代码 | `perfmgr_remap.c:95, 127` |

---

## 5. DCVS 仲裁 (dcvs_arbi/dcvs_main.c)

### 5.1 算法

该模块为纯算法/表决库，无自己的采样源，由外部 memlat 驱动回调。主要函数：

1. **`mi_record_pre_sampling_stats`**：采样窗口开始时快照 BE_STALL/CYC 计数，计算 `pre_be_stall_pct`

2. **`mi_calculate_sampling_stats`**：与基线做相对差分，锁存决策标志：
   - `is_be_stall`：be_stall_pct 相对增长 >10%
   - `is_be_ok`：be_stall_pct 相对下降 >10%
   - `is_ipm`：DDR 组 ipm 相对下降 >10%
   - `is_wb`：wb_pct >40%

3. **`mi_calculate_mon_sampling_freq`**：在 DDR 频率表中找到最近档位，按状态上提/下压

4. **FP 地板投票**：`mi_update_memlat_fp_vote` / `mi_update_memlat_fp_vote_revert` 实现 600MHz↔1500MHz 迟滞带

### 5.2 发现的问题

| 严重度 | 问题 | 证据 |
|--------|------|------|
| **高** | **`mult_frac` 除零**：`pre_be_stall_pct` 在干净窗口为 0，`mult_frac(100, Δ, 0)` 整型除零 Oops | `dcvs_main.c:195, 203, 214` |
| **高** | **`mult_frac` 除零**：`pre_cyc` 为 0 时 `mult_frac(100, pre_be_stall, 0)` 除零 | `dcvs_main.c:176` |
| **高** | **`mi_cpufreq_to_memfreq` 越界读 + 死代码**：`map-1` 在表首越界，`map--` 只改局部指针，void 函数对外无效 | `dcvs_main.c:248-253` |
| **中** | `mi_recalculate_freq_mhz` 除零 (`delta_us==0`) + 忽略 `freq_mhz` 参数 | `dcvs_main.c:236` |
| **中** | 锁设计失效：写锁被注释，全局基线 `pre_be_stall/cyc/ipm` 在 record 与 calculate 间无互斥 | `dcvs_main.c:117, 84-92, 165-228` |
| **中** | disable 瞬间无条件 `stat_data(0)` 撤票，与外部 DCVS 状态机兼容性未知 | `dcvs_main.c:327-332` |
| **低** | sysfs 属性在部分初始化失败时泄漏 | `dcvs_main.c:466-476, 487-493` |
| **低** | 决策标志在 disable/partial_on 切换间残留陈旧状态 | `dcvs_main.c:191-192, 89-92, 274-289` |

---

## 6. 入口链路与触摸

### 6.1 connected_buffer 管理

`connected_buffer_list` 链表最多管理 8 个节点。`perfmgr_notify_connect_cb` 负责节点查找/创建/复用。

**关键发现**：`is_rendering` 字段在 `reset_buffer()` 和 `init_buffer()` 中初始化为 0，但**全代码片任何地方从未将其置为 1**。导致：
- `perfmgr_set_buffer` 的超时检测永远不触发
- `single_layer` 过滤永远不拦截
- 节点复用判断恒为 true，永远复用第一个空闲节点
- **8 节点设计退化为单节点**，多 App 渲染时反复回收 node[0]

### 6.2 perfmgr_is_enable 延迟启用

```
perfmgr_enable=0 → 标记 need_delayed_enable=1, 返回 0
perfmgr_enable=1 + need_delayed_enable=1 → 累计计数, 15s 超时后启用
perfmgr_enable=1 + need_delayed_enable=0 → 直接返回 1
```

### 6.3 speed_touch 触摸提频

根据 `sf_available_buffer_size` 切换 producer/consumer 提频：

| buffer 数 | 阶段的提频对象 |
|----------|----------------|
| 0 | 提升 UI+Render，恢复 SF+HWC |
| 1 | 全部恢复 |
| ≥2 | 提升 SF+HWC，恢复 UI+Render |

### 6.4 发现的问题

| 严重度 | 问题 | 证据 |
|--------|------|------|
| **高** | `is_rendering` 永远为 0，渲染状态跟踪完全失效 | `perfmgr_main.c:113, 126, 143, 181, 245` — 仅写 0 |
| **高** | `perfmgr_notify_connect_fp` 函数指针从未被赋值 | `perfmgr_ioctl.c:28` 声明，`perfmgr_main.c:408` 未赋值 |
| **中** | `perfmgr_notify_connect` 声明但未定义 | `perfmgr_main.c:56` |
| **中** | 头文件中 `static struct list_head` 声明错误，每个 .c 文件独立实例 | `perfmgr.h:128` vs `perfmgr_main.c:49` |
| **中** | `connected_buffer` 链表操作 TOCTOU 竞态（两次遍历间释放锁再获取） | `perfmgr_main.c:233-240, 243-254` |
| **低** | `perfmgr_alloc_atomic` 分配大小与使用类型不匹配 | `perfmgr_main.c:336` |
| **低** | `f_rescue_minfreq` (1800000) 声明但未引用 | `perfmgr_policy.c:208-210` |
| **低** | `sf_comp_hint_enable` (SF completion) 为 TODO 未实现 | `perfmgr_policy.c:252-253` |
| **低** | `BUFFER_DEQUEUE(3)`、`BUFFER_VSYNC(5)`、`BUFFER_TOUCH(10)` 定义但未处理 | `perfmgr.h:131-133` |

---

## 7. 跨模块交互与数据流

### 7.1 外部接入点

| 接口 | 方向 | 语义 |
|------|------|------|
| `perfmgr_get_gpu_hook` (EXPORT_SYMBOL) | kgsl → perfmgr | 每次查询 target 频时覆盖 `*freq` |
| `perfmgr_notify_gpu_freq_fp` (EXPORT_SYMBOL 指针) | perfmgr → kgsl | 提频后回调通知 kgsl 重算 |
| `get_perfmgr_devfreq` (EXPORT_SYMBOL) | devfreq → perfmgr | 注入 perfmgr_df、gpu_freq_table |
| `boost_dcvs_freq(freq, hw_type)` | perfmgr → DCVS 驱动 | 实际写 DDR/L3 频率 |
| `stat_data()` | dcvs_arbi → AOSS/dcvs | 写 FP 地板投票 |
| `gpu_get_target_fps` (extern) | perfmgr_policy → perfmgr_gpu | 仅 ==60 时 GPU 模块生效 |
| dcvs_arbi 8 个导出符号 | memlat → dcvs_arbi | 采样回调 |

### 7.2 关键耦合点

1. **`gpu_get_target_fps`** 是全局变量，`perfmgr_do_policy` 对**每个 buffer**无条件写入，多 App 时"最后写者赢"，GPU 模块开关被所有 App 的帧共同决定。

2. **`target_gpu_freq`** 在 perfmgr_main.c 切换 buffer 时被复位为 0 并清零 `perfmgr_gpu_cur_history_items`。

3. **`cur_work`（含 hrtimer）是单个全局对象**，所有 buffer 共享，App B 的一帧会 cancel/重武装 App A 的保持定时器。

4. **dcvs_arbi 与 remap 同域并存**且互不感知，DDR 频率同时受两个模块影响。

---

## 8. 完整参数表

### 8.1 CPU 帧策略参数 (perfmgr_policy.c)

| 参数 | 默认值 | 作用 |
|------|--------|------|
| `perfmgr_enable` | 0 | 主开关，0=禁用，1=启用(15s 延迟) |
| `debug_mask_perfmgr` | 0 | 调试日志掩码 |
| `better_perf` | 1 | 1=ceiling+floor+hrtimer 三级释放；0=仅 max 限频 |
| `f_t_fps` | -1 | 固定目标 fps，-1=自动检测 |
| `scaling_r_thres` | 700 | jank 阈值偏移：`1e6 + 700×1000 = 1,700,000` |
| `r_perf` | 1 | 救援模式：0=两级，1=五级 |
| `r_step` | 750 | 救援步长阈值 (×1000) |
| `r_freq_level` | 0 | 救援档位偏移 |
| `scaling_a` | 380 | 预测提频阈值：`5e6 + 380×1000 = 5,380,000` |
| `scaling_a_thres` | 550 | 预测提频二级阈值 |
| `p_perf` | 0 | 预测模式：0=两级，1=三级 |
| `p_step` | 750 | 预测步长阈值 |
| `p_freq_level` | 1 | 预测档位偏移 |
| `scaling_b` | -50 | 降频阈值：`5e6 - (-50×1000) = 5,050,000` |
| `scaling_c` | 3 | 降频冷却：`(Δt)×fps > 3×1e6` |
| `nor_f_keep` | 12 | 正常模式保持帧数 |
| `cons_no_j_cnt` | 10 | 连续无卡顿帧数阈值 |
| `j_f_k_count` | 25 | jank 后额外保持帧数 |
| `perf_count` | 1 | 重武装 hrtimer 阈值 |
| `fast_down_freq_level` | -2 | 快速降频步进，-1=关闭 |
| `fast_down_circle_base` | 5 | 快速降频圈数基数 |
| `fast_down_level_thres` | 50 | 快速降频生效频级上限 |
| `max_freq_limit_level` | 27 | 频率档位索引上限 |
| `min_freq_limit_level_limit` | 0 | 频率档位索引下限 |
| `f_minfreq` | 384000 | 地板频率 (kHz) |
| `f_left_minfreq` | 384000 | 释放后地板频率 (kHz) |
| `b_minfreq` | 0 | 是否启用 min_freq 控制 |
| `cpu5_offset` | 0 | 小核簇频率偏移 (kHz) |
| `cpu7_offset` | 0 | 大核簇频率偏移 (kHz) |
| `l_cpu_start` | 0 | 策略生效起始 CPU |
| `timeout_144` | 9100 | 144fps 保持超时 (ms) |
| `timeout_120` | 13000 | 120fps 保持超时 (ms) |
| `timeout_90` | 16600 | 90fps 保持超时 (ms) |
| `timeout_60` | 25000 | 60fps 保持超时 (ms) |
| `timeout_49` | 27000 | 49fps 保持超时 (ms) |
| `timeout_30` | 44000 | 30fps 保持超时 (ms) |
| `timeout` | 6000 | 基础超时 (better_perf=0 时用) |
| `timeout_left` | 2333 | 三级释放后两阶段超时 (ms) |
| `timeout_r_freq_level` | 2 | 超时后降低档位数 |
| `load_reset` | 1 | 提频后是否重置滑动窗口 |
| `load_scaling_x` | 10 | load_reset 右移位数 |
| `load_scaling_y` | -1 | load_reset 偏移量 |
| `t_fps_49/61/91/121` | 0 | 启用细分 fps 档位 |
| `buffer_bypass` | 1 | 是否跳过 surfaceflinger 过滤 |
| `buffer_stop` | 0 | 是否拦截 surfaceflinger |
| `single_layer` | 0 | 仅允许单层渲染 |
| `buffer_timeout_us` | 80000 | buffer 超时时间 (μs) |

### 8.2 GPU 参数 (perfmgr_gpu.c)

| 参数 | 默认值 | 作用 |
|------|--------|------|
| `gpu_iffm_enable` | 0 | GPU IFFM 总开关 |
| `gpufreq_timeout_ms` | 1500 | 提频后释放延时 (ms) |
| `set_freq_direct` | 1 | 1=hook 直接覆盖 target；0=QoS 垫底 |
| `rescue_time_60` | 16,000,000 | 60fps 救援线 (ns) ≈ 一帧预算 |
| `unit_rescue_time_60` | 0 | 5 帧平均救援预算 (ns) |
| `release_time_60` | 0 | 单帧快降线 (ns) |
| `unit_release_time_60` | 10,500,000 | 5 帧平均快降线 (ns) |
| `target_gpu_freq` | S32_MAX | 当前目标 GPU 频率 (Hz) |
| `gpu_reset_freq` | 0 | 释放频率 |
| `debug_mask_gpu` | 0 | 调试掩码 |

### 8.3 DDR/L3 参数 (perfmgr_remap.c)

| 参数 | 默认值 | 作用 |
|------|--------|------|
| `d_boost` | 0 | DDR 频率联动开关 |
| `l3freq_boost` | 0 | L3 频率联动开关 |
| `dcvs_timeout_ms` | 1500 | DDR/L3 提升后释放延时 (ms) |
| `d_table` | 4 档 CPU→DDR | 见 4.1 节 |
| `l3freq_remap_table` | 4 档 CPU→L3 | 见 4.1 节 |

### 8.4 DCVS 仲裁参数 (dcvs_main.c)

| 参数 | 默认值 | 作用 |
|------|--------|------|
| `is_dcvs_arbi_enable` (sysfs) | false | 全局主开关 |
| `is_dcvs_arbi_partial_on` (sysfs) | false | 绕过标志 |
| `FLOOR_FREQ` | 600000 kHz | FP 地板投票触发线 |
| `CEIL_FREQ` | 3000000 kHz | cpufreq→memfreq 钳制上界 |
| `QUIT_FREQ` | 1500000 kHz | 投票退出线 |
| `ddr_freq_table[20]` | DT 解析 | DDR 频率档位表 |

---

## 9. 遗留风险与调优建议

### 9.1 按严重度排序的遗留风险

| 级别 | 风险 | 触发路径 | 影响 |
|------|------|----------|------|
| 🔴 **高** | **GPU 空指针崩溃**：`perfmgr_df->dev.parent` 在 devfreq 未注册时解引用 | 帧 END 快降 boost / hrtimer 回调 | 内核 panic |
| 🔴 **高** | **dcvs_arbi 除零 Oops**：`mult_frac(100, Δ, 0)` 在干净窗口整型除零 | 外部 memlat 每窗口回调 | 内核 panic |
| 🔴 **高** | **CPU hotplug 锁泄漏**：`update_policy_online` 错误路径未 `put_online_cpus()` | 任意 CPU 无 policy | 后续死锁 |
| 🔴 **高** | **`cpufreq_cpu_get` 引用泄漏**：perfmgr_policy_init 两处 get 无 put | 初始化路径 | 模块卸载泄漏 |
| 🔴 **高** | **多 App 状态坍缩**：`is_rendering` 恒 0 + 全局 static 变量 + 单例 hrtimer + GPU static 变量 | 双 App 并行渲染 | 频率决策互相覆盖 |
| 🟡 **中** | **hrtimer 原子上下文调可睡眠函数**：`dev_pm_qos_update_request`（mutex）| 帧 START 后 rescue 超时 | `scheduling while atomic` BUG |
| 🟡 **中** | **GPU rescue 逻辑死路**：预算耗尽返回 0 → hrtimer 不启动 | 前 4 帧累计超 5×unit_rescue_time | 预提频永久失效 |
| 🟡 **中** | **QoS 垫底自释放**：刚设的垫底频被同函数立即清除 | 每次 boost | 垫底逻辑形同虚设 |
| 🟡 **中** | **乱序帧 END 丢弃**：三级缓冲下帧测量丢失 | 多缓冲游戏 | 快降/救援判定丢失 |
| 🟡 **中** | **remap 错误路径泄漏**：`kstrdup` 后失败直接 return | sysfs 写入 d_table | 内存泄漏 |
| 🟡 **中** | **dcvs_arbi 死代码**：`mi_cpufreq_to_memfreq` 中 `map--` 只改局部指针 | 编译期 | 函数无效 |
| 🟡 **中** | **锁设计失效 + 数据竞争**：写锁被注释；GPU 全局变量无同步 | 多线程 GPU hint / 并发采样 | 决策失真 |
| 🟡 **中** | **`mi_cpufreq_to_memfreq` 越界读**：`map-1` 在表首解引用 | 传入表首元素 | 未定义行为 |
| 🟢 **低** | 代码死区：`gpu_load_history` 永不写入、`f_rescue_minfreq` 未引用、`sf_comp_hint_enable` 未实现、`BUFFER_DEQUEUE/VSYNC/TOUCH` 未处理等 | 编译期 | 无运行时影响 |

### 9.2 调优建议

**CPU 侧**：
- **启用顺序**：先 `perfmgr_enable=1`，15s 延迟生效后确认帧率识别正确
- **卡顿敏感度**：`scaling_r_thres=700` 调高降误报、调低增灵敏。`r_perf=1` 五级救援更激进
- **预测提频**：`scaling_a=380` → 调高（如 500）减少不必要的提前提频
- **降频节奏**：`scaling_c=3` → 5 拉长降频冷却，避免频繁升降抖动
- **高刷场景**：确认 `timeout_144/120` 与帧预算匹配；`f_t_fps` 固定值可跳过 30 帧学习期
- **地板**：`f_minfreq=384000` 偏低，游戏可抬到 614400 减少帧尾抖动
- **注意**：`max_freq_limit_level` 勿设 >27（会被一次性钳回 27）

**GPU 侧**：
- `gpu_iffm_enable=1` **仅在 `gpu_get_target_fps==60` 时生效**，120/144Hz 游戏 GPU 模块整体禁用。若需支持需扩展代码
- 先确认 `get_df_success>0` 再开 `gpu_iffm_enable`，否则空指针崩溃
- `gpufreq_timeout_ms=1500ms` 提频保持时长，过高费电、过低抖动

**DDR/DCVS 侧**：
- `d_boost/l3freq_boost` 默认关闭，内存带宽敏感型游戏可开
- `dcvs_timeout_ms=1500ms` 调大减少 DDR 反复升降
- **dcvs_arbi 与 remap 同域并存且互不感知**，实测若出现 DDR 频率互踩，应二选一启用

### 9.3 结论

该子系统是一套完整的"帧节奏驱动 × 频率域分层"设计：CPU 策略负责 jank 检测与档位状态机，GPU IFFM 用 ns 域 5 帧窗口做帧内预提频/快降，DDR/L3 通过 remap 表联动，dcvs_arbi 独立仲裁 memlat 采样。架构立意清晰，但工程完成度不足。**仅适合在单游戏前台、switched 开关显式开启、devfreq 就绪的受控环境下运行**。交付前应优先修复标"高"的 4 类问题：空指针、除零、锁泄漏、多 App 状态共享。