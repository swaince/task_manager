# 进程管理器 · Process & Port Inspector

一个用 **Tauri 2 + Vite + Vue 3 + TailwindCSS v4 + shadcn-vue** 构建的桌面应用，
用来查看本机进程、它们占用的端口、启动参数与父子关系，并支持按名称 / 端口过滤和结束进程。

当前版本以 **Windows** 为目标平台，但后端从第一行代码起就按「可扩展到 macOS / Linux」来设计
（详见 [跨平台扩展](#跨平台扩展)）。

---

## 功能一览

| 能力 | 说明 |
| --- | --- |
| 进程列表 | 图标、名称、PID、父 PID、状态、CPU、内存、线程 / 句柄、用户、命令行 |
| 端口视图 | `netstat -ano` 式的全量套接字列表：端口、协议、状态、本地/远端地址、所属进程，可按端口/协议/状态/进程排序 |
| 端口过滤 | 结构化表达式（见下）：单端口、多端口、区间、`tcp:`/`udp:`、`local:`/`remote:`；可点端口徽标一键增删 |
| 监听端口 | TCP `LISTEN` 与 UDP 绑定端口高亮标记；详情面板可单独筛选 |
| 名称搜索 | 匹配进程名、PID、可执行路径、完整命令行、用户名、窗口标题、端口号 |
| 快捷过滤 | 「仅监听端口」「仅有窗口」开关；端口视图下为「仅监听中 / 仅活跃连接」+ TCP/UDP 协议筛选 |
| 进程树 | 基于父 PID 构建，可逐级展开 / 全部展开，过滤时自动保留匹配项的祖先链并对无关节点置灰 |
| 详情面板 | 概览 / 端口（可再筛选）/ 命令行（含参数拆解）/ 环境变量 / 关系（祖先链、子进程、窗口列表） |
| 结束进程 | 单进程或整棵进程树，二次确认 + 强制 / 优雅终止可选；成功失败均有轻提示 |
| 其他操作 | 切换到前台窗口、在资源管理器中定位、复制 PID / 命令行 / 进程详情 |
| 采样控制 | 0.5s ~ 30s 可调刷新间隔、暂停 / 恢复、手动立即刷新 |
| 键盘操作 | `↑` `↓` 切换、`Enter` 定位窗口、`Delete` 结束进程 |

---

## 端口过滤语法

端口输入框接受一组用逗号 / 空格 / 分号分隔的表达式，**前缀可叠加，顺序不限**：

| 写法 | 含义 |
| --- | --- |
| `8080` | 端口等于 8080 |
| `80,443` | 多个端口（也支持空格、`；`、`、` 分隔） |
| `8000-8100` | 端口区间 |
| `tcp:8080` / `udp:53` | 限定协议 |
| `local:3000` / `l:3000` | 只匹配**本地**端口 |
| `remote:443` / `r:443` | 只匹配**远端**端口 |
| `tcp:local:8000-8100` | 前缀叠加 |

行为细节：

* 未写 `local:` / `remote:` 时，按工具栏的**作用域选择**决定匹配哪一端（本地+远端 / 仅本地 / 仅远端）。
* 多组表达式之间是 **OR**；一组表达式内部（区间）是范围匹配。
* 无法识别的片段会被静默忽略，所以边输入边过滤不会因为半成品表达式而闪空。
* 表达式生效时，工具栏与状态栏都会显示规范化的规则徽标，点 `×` 即可单独移除。
* **点任何端口数字都能一键增删该端口的过滤条件** —— 进程行里的端口徽标、端口视图的本地端口、
  详情面板的本地地址，点击后按**本地端口**过滤；端口视图与详情面板**远程地址里的端口**同样可点，
  会自动带上 `remote:` 前缀（这样即便工具栏选着「仅本地端口」也能立刻命中）——
  从「某个进程占了 8080」到「谁还在用 8080」只需一次点击。
* 端口视图下过滤是**逐条套接字**匹配；进程列表视图中，只要进程有任意一条套接字命中，该进程就会显示。

### 「本地 / 远端」到底指什么

一条 TCP 套接字由四元组唯一确定：`本机地址:本地端口 ↔ 远端地址:远端端口`。
**本地端口**是本机这一端占用的端口，**远端端口**是对端的端口 —— 同一个数字 `443` 在两侧含义完全不同：

| 场景 | 本地 | 远端 | 状态 |
| --- | --- | --- | --- |
| nginx 监听 443 | `0.0.0.0:443` | — | LISTEN |
| Chrome 打开 https 网站 | `192.168.1.10:51234` | `93.184.216.34:443` | ESTABLISHED |
| 某程序连本机 Redis | `127.0.0.1:52001` | `127.0.0.1:6379` | ESTABLISHED |

于是输入 `443` 时：

| 作用域 | 命中 | 回答的问题 |
| --- | --- | --- |
| **仅本地端口** | 只有 nginx | 谁**占用**了 443 |
| **仅远端端口** | 只有 Chrome 等连出去到 443 的进程 | 谁在**访问**远端的 443 |
| **本地+远端**（默认） | 两者都显示 | 一起答，结果最多 |

UDP 套接字没有远端端口，因此「仅远端」对 UDP 永不命中。

实现见 `src/lib/port-filter.ts`（纯函数，20 个单测覆盖），排序/聚合见 `useProcessStore.ts`。

---


## 技术栈与目录结构

```
task_manager/
├─ src/                          前端（Vue 3 + TS + Tailwind v4 + shadcn-vue）
│  ├─ api/process.ts             所有 Tauri 命令的薄封装（组件不直接 invoke）
│  ├─ components/
│  │  ├─ layout/                 AppHeader、AppStatusBar
│  │  ├─ process/                工具栏、进程网格、端口网格、详情面板、端口徽标、确认弹窗
│  │  └─ ui/                     shadcn-vue 生成的组件（button/table/tabs/dialog/...）
│  ├─ composables/useProcessStore.ts   唯一状态源：轮询、过滤、排序、进程树、端口视图、写操作
│  ├─ lib/port-filter.ts         端口表达式解析 / 匹配（纯函数，带单测）
│  ├─ lib/format.ts              纯格式化函数（字节 / 时长 / 状态…）
│  └─ types/process.ts           与 Rust `model.rs` 一一对应的类型
│
└─ src-tauri/                    后端（Rust）
   └─ src/
      ├─ commands.rs             Tauri IPC 边界
      ├─ service.rs              采样调度：后台线程 + 缓存 + 写操作后立即重采
      ├─ platform/
      │  ├─ mod.rs               `ProcessProvider` trait + 平台选择
      │  ├─ provider.rs          平台无关的编排（sysinfo + netstat2）
      │  ├─ win.rs               Windows 原语：提权判定、窗口枚举、图标抽取…
      │  └─ fallback.rs          非 Windows 的占位实现（扩展入口）
      ├─ model.rs                DTO
      └─ error.rs                统一错误类型
```

**为什么这样分层**

* 数据来源有三块，天然需要合并：进程属性（`sysinfo`）、网络套接字（`netstat2`）、
  平台独有信息（窗口 / 权限 / 图标）。
* 前端不参与任何「采集」逻辑，只做展示与交互；后端不参与任何「过滤 / 排序」逻辑，
  只负责如实、尽快地给出数据。边界清晰，替换任一侧都不影响另一侧。
* 图标的 DataURL 单独按 `exe` 路径惰性拉取并双向缓存（后端 + 前端），
  避免每次轮询都传输几百 KB 的 base64。

---

## 数据流

```
后台采样线程（service.rs）
    每 N 毫秒 → provider.snapshot()
        ├─ sysinfo  刷新内存 / CPU / 进程（轻量刷新，命令行等只在首次读取）
        ├─ netstat2 读取 TCP/UDP 套接字表，按 PID 归组
        └─ native   枚举窗口标题、探测进程可读性、读句柄数
    → 写入 RwLock 缓存

前端（useProcessStore.ts）
    每 N 毫秒 invoke('list_processes')  → 读缓存（零等待）
    用户点开某进程 → invoke('get_process_detail', { pid }) → 该进程全量刷新（含环境变量）
    结束进程后 → 后端立刻重采一次，前端同时再拉一次
```

CPU 使用率依赖两次采样的差值，因此 `ProcessService::new()` 会在首帧采样两次
（中间 `sleep(MINIMUM_CPU_UPDATE_INTERVAL)`），避免第一屏全是 0%。

---

## 开发与构建

```bash
pnpm install                  # 安装前端依赖
pnpm tauri dev                # 开发模式（热更新，打开桌面窗口）
pnpm tauri build              # 打包安装包（Windows 下产出 NSIS 安装程序）
pnpm build                    # 仅构建前端产物到 dist/
pnpm test                     # 前端单测（端口过滤表达式解析 / 匹配）
cd src-tauri && cargo test    # 后端单元测试（14 个）
```

> 建议以 **管理员身份** 运行：普通权限下，受保护进程（系统进程、其他用户会话下的进程、
> 带保护的服务）读不到命令行 / 环境变量，也无法结束。这类进程会在列表里标记为「受保护」。

---

## 关键实现说明

### 端口 → 进程的映射

`netstat2` 在 Windows 上走 `GetExtendedTcpTable` / `GetExtendedUdpTable`，
返回的每条记录自带 `associated_pids`。我们把「一个套接字」复制给它的每个 PID，
再为每个进程汇总出去重的 `ports` 与 `listeningPorts`。

* TCP 只有 `state == Listen` 才算监听；
* UDP 没有 `LISTEN` 状态，**绑定了端口即视为监听候选**（状态标记为 `BOUND`）；
* IPv4-mapped 的 IPv6 地址（`::ffff:127.0.0.1`）会被还原成 `127.0.0.1`，便于阅读与过滤。

### 端口过滤的两条数据路径

同一份套接字数据被两种视图消费，过滤语义保持一致（都走 `lib/port-filter.ts` 的纯函数）：

* **进程列表 / 进程树**：进程有任意一条套接字命中规则即保留 —— 回答「谁在用这个端口」。
* **端口视图**：逐条套接字匹配后再附带宿主进程 —— 回答「这个端口上有哪些连接」。

点击端口数字触发 `togglePortFilter()`：它先把当前表达式解析成规则数组，
增删目标端口的精确规则后再用 `ruleToToken()` 规范化回写输入框，
因此用户手写的区间与协议前缀不会被这个交互破坏。

### 进程树

后端只提供 `parentPid` 与 `children`，树的构建、展开、以及「过滤时保留祖先链并对
无关节点置灰」全部在前端完成（`buildTree` / `treeSource`）。这样切换视图不需要任何请求。

### 结束进程树

`GenericProvider::descendants()` 先在快照里收集「自身 + 全部后代」，
按**深度由深到浅**排序后依次结束，避免父进程先退出导致子进程被系统重新挂到别处。

### 图标抽取（Windows）

`SHGetFileInfoW(SHGFI_ICON | SHGFI_LARGEICON)` 拿到 `HICON` →
`GetIconInfo` 取 `hbmColor` → `GetObjectW` 读尺寸 → `GetDIBits` 读 32 位 BGRA →
转 RGBA（全 0 alpha 的图标用颜色推断可见性）→ `image` 编码 PNG → base64 DataURL。
结果按 exe 路径缓存在进程内。

---

## 跨平台扩展

后端已经把所有平台差异收敛到 **一处**：`platform::native`（当前是 `win.rs`）。

新增一个平台只需要：

1. 新建 `src-tauri/src/platform/mac.rs`（或 `linux.rs`），实现与 `win.rs` **完全相同签名** 的函数：

   | 函数 | 作用 | 建议实现 |
   | --- | --- | --- |
   | `platform_id()` | 平台标识 | 返回 `"macos"` / `"linux"` |
   | `is_elevated()` | 是否管理员 / root | macOS / Linux：`geteuid() == 0` |
   | `is_process_accessible(pid)` | 能否读取受保护信息 | Linux：`/proc/<pid>/` 可读性；macOS：进程鉴权状态 |
   | `window_titles()` | PID → 窗口标题 | macOS：`CGWindowListCopyWindowInfo`；Linux：X11 `_NET_WM_PID` |
   | `handle_count(pid)` | 句柄 / 文件描述符数 | Linux：统计 `/proc/<pid>/fd` 条目数 |
   | `icon_data_url(exe)` | 图标 DataURL | macOS：读 `.app` 的 `Info.plist`；Linux：解析 `.desktop` 的 `Icon=` |
   | `focus_window(pid)` | 窗口切到前台 | 各平台对应 API |
   | `reveal_in_file_manager(path)` | 文件管理器定位 | macOS：`open -R`；Linux：`xdg-open` 父目录 |

2. 在 `platform/mod.rs` 里把 `#[cfg(not(windows))]` 的模块路径指向新文件。

`provider.rs`、`service.rs`、`commands.rs` 与**整个前端**都不需要改动 ——
数据模型（`model.rs`）本身已经是平台中立的，平台独有字段一律是 `Option`。

---

## 已知限制

* 结束进程在 Windows 上没有真正的「优雅终止」（`SIGTERM`），
  关闭「强制结束」时 `sysinfo::Signal::Term` 不被支持，会自动退化为 `TerminateProcess`。
* 受保护进程（`PPL`、反作弊保护、其他用户会话）即使提权也可能读不到命令行。
* 端口归属依赖系统套接字表：某些内核态 / 驱动态监听端口不会出现在任何进程名下，
  这类端口在界面上（按设计）不展示。
* 环境变量只在打开详情面板时读取，因此列表里不显示；受保护进程该页会提示为空。
