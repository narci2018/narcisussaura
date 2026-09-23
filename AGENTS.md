# AGENTS.md — Narcissus Aura 多平台协作规则

本仓库是 Tauri 2 应用(Android / Windows / macOS 三端发布)。
**架构决策(已经用户确认):目录级平台隔离** —— 同一仓库、共享内核,
每个平台有专属目录;CI 与发布流程不变。不做三份物理拷贝。

## 1. 目录地图

### 共享内核(改动需三家评估)
- `src-tauri/src/core/` — sing-box / mihomo 配置生成、协议适配
- `src-tauri/src/managers/` — 连接、节点、订阅等管理流程
- `src-tauri/src/models.rs`、`src-tauri/src/lib.rs` — 数据模型、命令注册
- `src/stores/`、`src/services/`、`src/types/` — 前端状态与 IPC 层(逻辑共享,UI 不在此列)
- `src/components/TitleBar.tsx`、`src/components/SimpleMode/` — 跨端共享 UI(小白模式三端同布局)

### 专家模式视图层:PC 与 Android 物理隔离(用户拍板的方案 B,不再共享)
- `src/platform/pc/expert/` — Windows + macOS 共用的专家模式全部视图
  (Navigation / Dashboard / ServerList / ChainedProxyView / PsiphonView / VPNGateView /
  ResidentialView / MegaVView / SubscriptionsView / ImportModal / SettingsView / QuoteBar / RelayBar)。
- `src/platform/android/expert/` — Android 独立同一套视图的拷贝;移动端 UI 适配只改这里。
- 两侧各有一份 `index.ts` 注册为 `ExpertLayer { Navigation, views: Record<ActiveTab, FC> }`,
  经 `PlatformProfile.expert` 由 App.tsx 消费。**改任何一侧,另一侧零影响;**
  同一 bug 两端都要修时,分别提交、分别前缀,禁止"顺手"跨拷。
- PC 端(win+mac)如两家出现分叉,再在 `src/platform/windows/`、`src/platform/macos/`
  里 spread 覆盖 `pcProfile`(现有做法),不另起第三份拷贝。

### 平台专属目录(该平台的问题只允许改这里)
| 平台 | Rust | 前端 | 其它 |
|---|---|---|---|
| Android | `src-tauri/src/platform/android.rs` | `src/platform/android/` | `mobile/android/*.kt`、`src-tauri/mobile/android/inject_vpn.py`、`tunrelay/`(gVisor 桥) |
| Windows | `src-tauri/src/platform/windows.rs` + `job_object.rs` + `win_proxy.rs` | `src/platform/windows/` | — |
| macOS | `src-tauri/src/platform/macos.rs` | `src/platform/macos/` | — |

`src/platform/index.ts` 是唯一分发点:组件只 `import { currentProfile } from '../platform'`,
禁止在组件里写 UA 嗅探 / `navigator.userAgent` 判断 / `isAndroid` 之类散落的平台检测。

## 2. 硬规则

1. **平台 bug 修复只允许落在该平台的目录里。**
   只改一个平台时,共享区文件一行都不许动;把逻辑收进对应平台文件。
2. **共享区只允许"薄接缝"。**
   共享文件中的 `#[cfg(target_os = ...)]` 块内只允许 1~3 行对平台模块的调用
   (现有接缝:`managers/connection_manager.rs` 的调用点、`lib.rs` 的 crash 报告/panic hook、
   `core/singbox.rs` 的 `auto_detect_interface`)。
   接缝里长出任何业务逻辑 = 违规,先下沉到平台文件再调。
3. **共享区改动必须三家评估、三家验证。**
   提交信息前缀如实:`fix(android):` / `fix(windows):` / `fix(macos):` / `fix(all):`。
   影响多平台的共享改动必须写 `fix(all)` 并说明三家如何验证。
   教训:v0.2.96→0.2.97,dns hijack 规则形状问题在 android 与 desktop 各炸一次,
   就是因为修复只按单平台形状落地。
4. **不许"顺手"修另一个平台。** 发现别的平台有同样问题,单独记录、单独修复、单独前缀。
5. **sing-box 兼容性以 CI 钉住的版本为准**(见 memory:sing-box 1.14.x 对齐);
   生成的配置改动必须过本地 `sing-box check`(见 §3)。

## 3. 每次发布前的验证流程(按顺序,全部离线)

```bash
# 1) 主机测试(含 android 配置生成集成测试,自动产出 fixture)
cargo test --manifest-path src-tauri/Cargo.toml
# 2) android cfg 分支编译检查(主机测试从不编译这些分支;需本地 NDK)
CC_aarch64_linux_android="C:/Tools2/android-ndk/android-ndk-r26d/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang.cmd" \
CXX_aarch64_linux_android="C:/Tools2/android-ndk/android-ndk-r26d/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang++.cmd" \
AR_aarch64_linux_android="C:/Tools2/android-ndk/android-ndk-r26d/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-ar.exe" \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="C:/Tools2/android-ndk/android-ndk-r26d/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang.cmd" \
cargo check --target aarch64-linux-android --manifest-path src-tauri/Cargo.toml
# 3) Kotlin 侧(手机 app 的 java/kotlin 改动)
bash .ktcheck/ktcheck.sh
# 4) 手机形状的配置静态验证
sing-box check -c src-tauri/target/android_smart_config.json
```

Windows/macOS 共享改动由 CI 的 `workflow_dispatch`(不发布、只构建)兜底验证;
android 由上述本地交叉检查兜底。

## 4. 调试纪律(用户明确要求)

- **禁止建议 ADB / 连真机调试。** 手机侧诊断只走 app 内「拷贝完整日志」。
- 先读代码找根因,再动手修;不许"试一版上手机看"。
- 如实汇报:不确定就说不确定,没验证就说没验证。

## 5. 版本与发布

- git tag 版本必须与四处一致:`package.json`、`src-tauri/Cargo.toml`、
  `src-tauri/tauri.conf.json`、`src-tauri/Cargo.lock`(`tauri-app` 条目)。
- CI 由推送 `v*` tag 触发(见 `.github/workflows/build-crossplatform.yml`);
  tag 推送用 `git -c http.proxy= -c https.proxy= push origin main vX.Y.Z`。
- `gh` 不可用时用匿名 REST API;GitHub release 资源按 tag 查询有 ~25-40 分钟延迟,
  判断"是否真的没传上"要用 `/releases/{id}/assets` + PowerShell HEAD 双重确认。
- **禁止把 scratch 文件加入 git**:`src-tauri/target/` 下的实验产物、`.sb-a64/`、`.ktcheck/` 输出、
  `v2ray_sub.txt`、`.cred.txt` 等。
- 凭据一律 `printf "protocol=https\nhost=github.com\n\n" | git credential fill`,禁止 echo。

## 6. 迁移新平台代码的指引

新增平台差异代码时:
1. 先在对应平台的 Rust/前端目录里写实现;
2. 若需要被共享流程调用,在共享文件里只加一处 cfg 薄接缝(§2.2);
3. 接缝两侧(android vs 非 android)的错误信息若重复,提炼成平台内单一函数
   (例:`ConnectionManager::android_vpn_consent_error`)。
