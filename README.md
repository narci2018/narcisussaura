# Narci'ssus Aura (水仙光环)

<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="Narcissus Aura Logo" width="96" height="96" />
</p>

<p align="center">
  <strong>一款基于 Tauri 2 + Rust + React 开发的现代化、跨内核、多协议 Windows 代理与网络加速客户端</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2B%20(x64)-blue?style=flat-square&logo=windows" alt="Platform" />
  <img src="https://img.shields.io/badge/Tauri-v2.0-24C8D8?style=flat-square&logo=tauri" alt="Tauri" />
  <img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/React-18-61DAFB?style=flat-square&logo=react" alt="React" />
  <img src="https://img.shields.io/badge/Sing--Box-v1.14.0-black?style=flat-square" alt="Sing-Box" />
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License" />
</p>

---

> **致敬推动文明进步的每一个人。**  
> 他们争取到的光，会穿越漫长岁月，照亮世间每一个人，包括你。 —— 无名氏  
> *A tribute to everyone who has helped advance human civilization. The light they fought for will travel across the ages, illuminating the lives of everyone in this world—including you.* — Anonymous

---

## 🌟 核心特性 (Key Features)

### 1. 🚀 多内核聚合与全协议覆盖
- **内置 Sing-Box 核心 (v1.14.0)**：完整支持 VLESS (Reality/WS/gRPC)、VMess、Trojan、Shadowsocks、Hysteria 2、WireGuard 等主流及新兴抗封锁协议。
- **内置 Psiphon 赛风专用核心**：原生集成 Psiphon 混淆隧道，具备动态节点探测、多国节点可用数展示与毫秒级智能切流。
- **内置 VPNGate 免费学术中继**：实时抓取日本筑波大学 VPNGate 全球开源节点镜像，支持一键载入与测速连接。
- **内置 MegaV / Aether (MASQUE) 核心**：原生驱动 Cloudflare MASQUE / WARP-in-WARP / TLS ClientHello 乱序分段混淆与 ECH (Encrypted Client Hello)，无视深度包检测 (DPI)。
- **高性能 TUN 虚拟网卡**：集成微软认证签名的 `wintun.dll` 驱动，支持全局系统级虚拟网卡转发。

### 2. 🔗 链式代理与跳板中转 (Chained Proxy)
- **多跳级联代理**：支持自由配置“跳板中转节点 (Relay Transit)”与“落地出口节点 (Landing Node)”。
- **灵活的 CRUD 可视化管理**：
  - 窗口尺寸自由缩放调节，适配各种屏幕尺寸；
  - 节点选择器内置“分组过滤”，便于从庞大的节点池中按地域或标签极速定位跳板；
  - 支持单跳和链式节点一键测速。

### 3. 🛡️ 智能化路由规则系统 (Smart Routing Rules)
- **原生 SRS 二进制规则集**：内置最新 `geosite-cn.srs`、`geoip-cn.srs`、`geosite-category-ads-all.srs`、`geosite-private.srs`。
- **路由规则集可视化编辑**：类似 v2rayN 的规则集管理体验，支持预设“默认”规则集以及自定义新建、修改、保存个性化路由集。
- **开箱即用规则策略**：
  - **国内直连 (Direct)**：精准匹配国内域名与 IP 段，访问大陆服务绝不绕路。
  - **私网与局域网直连 (Private Direct)**：局域网设备、路由器控制台、NAS 直达不走代理。
  - **广告与恶意追踪拦截 (Block Ads)**：内置阻止全网常见广告与跟踪域。
  - **阻断 UDP 443 (Block UDP 443 Switch)**：独立开关，阻断 QUIC 流量强制浏览器回退至 TCP/TLS，完美规避运营商恶意的 UDP 降速与 QoS 干扰。

### 4. 💻 现代化科幻界面与无缝 Windows 体验
- **Cyberpunk 视觉美学**：无边框暗黑发光玻璃拟态（Glassmorphism），搭配沉浸式动态状态指示光环。
- **丰富的数据统计**：实时刷新当前下行/上行速率、累计会话流量、已连接持续时间、目标节点物理坐标。
- **系统托盘深度集成**：
  - 点击窗口最小化（`-`）时，程序静默缩小至 Windows 系统任务栏托盘，不侵占任务栏空间；
  - **双击托盘图标**：快速恢复并聚焦窗口；
  - **右键托盘图标**：弹出快捷菜单（“打开主界面”、“退出应用”）。
- **友好错误提示**：连接失败或配置异常时，浮层一键拷贝完整报错文本，方便排查与反馈。

---

## 📂 项目结构 (Project Architecture)

```text
vpnclient/
├── .github/
│   └── workflows/
│       └── build-windows.yml       # GitHub Actions 自动化编译打包 NSIS exe 脚本
├── src-tauri/                       # Rust 后端核心与 Tauri 宿主进程
│   ├── binaries/                    # 【核心引擎】内嵌的可执行内核与规则（由 Git LFS 管理）
│   │   ├── rules/                   # SRS 二进制路由规则集 (geoip-cn, geosite-cn 等)
│   │   ├── sing-box.exe             # 全协议代理核心
│   │   ├── mihomo.exe               # Clash Meta 路由引擎
│   │   ├── psiphon-tunnel-core.exe  # 赛风混淆穿透核心
│   │   ├── aether.exe               # MASQUE / WARP 引擎
│   │   ├── wintun.dll               # Windows TUN 虚拟网卡驱动
│   │   └── server_entries.txt       # Psiphon 官方内置节点数据
│   ├── src/
│   │   ├── core/                    # 内核调用适配器 (singbox.rs 等)
│   │   ├── managers/                # 连接调度、链式代理、节点管理、测速模块
│   │   ├── platform/                # Windows 代理注册表操控、JobObject 进程生命周期守护
│   │   ├── lib.rs                   # Tauri 命令分发、托盘与窗口控制
│   │   └── main.rs                  # 应用程序主入口
│   ├── Cargo.toml                   # Rust 依赖清单 (tauri v2, tray-icon, windows 等)
│   └── tauri.conf.json              # Tauri 2 窗口、资源与打包配置
├── src/                             # 前端 React 18 + TypeScript 源码
│   ├── components/                  # UI 视图组件 (Dashboard, ServerList, ChainedProxy, TitleBar 等)
│   ├── stores/                      # Zustand 响应式全局状态管理
│   ├── services/                    # 前后端 Tauri IPC 通信封装
│   └── App.tsx                      # 主窗口路由与导航容器
├── .gitattributes                   # Git LFS 大文件追踪规则
├── .gitignore                       # 严密过滤临时文件与构建中间产物的忽略清单
├── package.json                     # 前端依赖配置
└── vite.config.ts                   # Vite 打包配置
```

---

## 🛠️ 本地开发与编译环境 (Development & Building)

### 前置要求 (Prerequisites)
1. **操作系统**：Windows 10 / 11 64位
2. **Node.js**：v18.0 或更高版本，包管理器推荐 **pnpm** (`npm i -g pnpm`)
3. **Rust 工具链**：最新 stable 版本，包含 `x86_64-pc-windows-msvc` 目标
4. **C++ 构建工具**：Visual Studio 2022 (勾选 “C++ 桌面开发”)
5. **Git 与 Git LFS**：克隆仓库需要安装 [Git LFS](https://git-lfs.com/)

### 1. 克隆源码并拉取 LFS 核心文件
```bash
# 克隆仓库
git clone https://github.com/<your-username>/vpnclient.git
cd vpnclient

# 初始化并拉取内核引擎二进制（必须执行，否则没有内嵌内核）
git lfs install
git lfs pull
```

### 2. 安装前端依赖
```bash
pnpm install
```

### 3. 本地启动热重载开发环境 (Dev)
```bash
pnpm tauri dev
```

### 4. 生产环境编译打包 (Release Build)
```bash
# 仅生成 NSIS 单文件安装包 (NarcissusAura_x.x.x_x64-setup.exe)
pnpm tauri build --bundles nsis
```
打包成功后，输出的安装程序位于：
`src-tauri/target/release/bundle/nsis/NarcissusAura_0.2.0_x64-setup.exe`（文件大小约为 46MB）。

---

## 🤖 GitHub Actions 自动化云端构建 (CI/CD)

项目已配置完毕 GitHub Actions 自动化工作流：[`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml)。

### 工作流特性：
1. **自动拉取 LFS 引擎**：使用 `actions/checkout@v4` 结合 `lfs: true` 自动下载完整的内核二进制。
2. **内核完整性校验**：构建前自动校验全部 10 项核心依赖，如果存在缺失或空的 LFS 指针会立即报警熔断。
3. **单目标构建**：仅编译 NSIS `.exe` 安装程序，节省近一半的 CI 时间。
4. **自动发布 Release**：当向仓库推送版本标签（例如 `git tag v0.2.0 && git push origin v0.2.0`）时，GitHub Actions 会全自动创建 Release 并将编译出的 `.exe` 安装程序附加供下载。

---

## 📋 常见问题 (FAQ)

#### Q: 为什么安装包体积在 46MB 左右？
A: 安装包内置了 Sing-Box、Mihomo、Psiphon、Aether 4 个完整且独立的外部代理内核以及微软签名的 Wintun 网卡驱动，未压缩前总体积超过 215MB。经 NSIS 极高压缩后约为 46MB，保证了用户开箱即用，无需额外配置或单独下载任何依赖。

#### Q: 如何开启开机自启或常驻后台？
A: 点击窗口右上角的最小化按钮即可无缝收纳到 Windows 任务栏系统托盘；如需彻底关闭客户端，在托盘图标上点击右键选择“退出”即可。

---

## ⚖️ 免责声明 (Disclaimer)

本项目仅供计算机网络技术研究、跨平台客户端架构学习以及网络安全合规测试使用。请使用者严格遵守所在国家或地区的法律法规，切勿用于任何违反法律或侵害他人合法权益的用途。作者不对使用本软件产生的任何后果承担任何责任。

## 📄 许可证 (License)

Distributed under the [MIT License](LICENSE).

