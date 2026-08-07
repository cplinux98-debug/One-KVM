# One-KVM（个人修改构建版）

> **原项目地址：https://github.com/mofeng-git/One-KVM**
>
> 本仓库是上述项目的 fork。One-KVM 的全部原始功能、设计与著作权均归原作者 [mofeng-git](https://github.com/mofeng-git) 所有。
> 本仓库仅包含个人的少量修改（见下方[本仓库的修改](#本仓库的修改)），并遵循原项目的 GPL 许可证发布。
>
> **安装方式、硬件准备、系统要求、USB OTG 配置等完整文档，请以原项目和 [One-KVM 官方文档站](https://docs.one-kvm.cn/) 为准，本仓库不再重复。**
> 如需使用原版请直接前往原项目仓库；遇到与本仓库修改无关的问题，请优先向原项目反馈。

---

## 运行本仓库构建的 Docker 镜像

镜像仓库：[hub.docker.com/r/cplinux98/one-kvm](https://hub.docker.com/r/cplinux98/one-kvm)

```bash
docker run --name one-kvm -itd \
  --privileged=true --restart unless-stopped \
  -v /dev:/dev -v /sys:/sys \
  --net=host \
  cplinux98/one-kvm:latest
```

启动后浏览器访问 `http://<设备IP>:8080`，首次访问会引导完成初始配置（创建管理员账号、选择采集设备等）。

### 可用标签

| 标签 | 说明 |
| --- | --- |
| `cplinux98/one-kvm:latest` | 最新构建 |
| `cplinux98/one-kvm:v0.2.6-wol` | 基于上游 v0.2.6，含本仓库的 WOL 修改 |

仅提供 `linux/amd64` 架构。ARM 设备（树莓派、派星等）请使用原项目的官方镜像。

### 持久化配置

容器内配置与数据位于 `/etc/one-kvm`，默认随容器销毁而丢失。建议挂载数据卷保留：

```bash
docker run --name one-kvm -itd \
  --privileged=true --restart unless-stopped \
  -v /dev:/dev -v /sys:/sys \
  -v /opt/one-kvm:/etc/one-kvm \
  --net=host \
  cplinux98/one-kvm:latest
```

### 使用网络唤醒功能

本仓库修改的核心就是这部分，使用步骤：

1. 进入 **设置 → ATX → 网络唤醒设置**
2. 打开 **启用网络唤醒** 开关
3. 按需填写发包用的网络接口（留空为自动选择）
4. 在 **唤醒目标** 中填写名称与 MAC 地址（最多 5 个，留空的行会被忽略），保存
5. 回到远程页面，点击顶部 **电源** 按钮，选择目标后发送唤醒包

即使 ATX 电源管理处于禁用状态，只要启用了网络唤醒，电源按钮就会保留并自动展示 WOL 面板。

---

## 本仓库的修改

基线：上游 [`a4073d6`](https://github.com/mofeng-git/One-KVM/commit/a4073d6)（v0.2.6）

### 解决的问题

原版中远程页面「电源」按钮的显示与否，**只取决于 ATX 电源管理是否启用**。这导致一个矛盾：网络唤醒本身是独立于 ATX 硬件的功能，但只要把 ATX 电源管理设为禁用，电源按钮就会整个消失，已配置好的网络唤醒也随之无法使用。

### 修改日志

#### 2026-08-07 — 网络唤醒独立开关与具名 WOL 目标

提交 [`54aa5f3`](https://github.com/cplinux98-debug/One-KVM/commit/54aa5f3)，共 19 个文件，+560 / −147。

**功能变更**

- **新增网络唤醒独立开关**（`wol_enabled`）。WOL 与 ATX 电源管理彻底解耦，仅当两者**都**禁用时，远程页面的电源按钮才会隐藏。
- **设置页支持保存最多 5 个具名唤醒目标**（名称 + MAC）。远程页面不再手工输入 MAC，改为直接从已保存的目标中选择发送。
- **优化远程页面电源弹窗**：ATX 禁用而 WOL 启用时不再显示 ATX 控制，自动切换到 WOL 面板；仅启用其中一项时不再显示多余的标签页。
- **MAC 地址统一归一化**为 `AA:BB:CC:DD:EE:FF` 格式，兼容冒号、短横线与无分隔符三种写法，并校验非法格式、重复地址与数量上限。
- **网络唤醒关闭时**，`/api/atx/wol` 接口拒绝发包并返回明确错误。

**接口变更**

- `/api/atx/status` 新增 `wol_enabled`、`wol_targets`、`power_controls_available` 三个字段。
- `/api/config/atx` 的 PATCH 支持 `wol_enabled` 与 `wol_targets`。

**缺陷修复**

- 修复 WebSocket `device_info` 事件在 ATX 控制器未初始化时，把整个 ATX 状态置空的问题。该问题会导致即便启用了网络唤醒，电源按钮也会在收到事件推送后消失。
- 修复 cross 交叉编译镜像在 `CHINAMIRRO=1` 下未替换 `debian-security` 源的问题：原正则要求 `debian` 后紧跟空白字符，`debian-security` 因此未被匹配，仍走 `deb.debian.org`，导致国内构建时 apt 下载极慢。

**升级注意**

`wol_enabled` 默认为 `false`。从原版升级后，网络唤醒面板不会自动出现，需要先到设置页手动开启。

### 验证情况

| 项目 | 结果 |
| --- | --- |
| 前端 `vue-tsc` 类型检查 | 通过 |
| Rust release 交叉编译 | 0 错误 0 警告 |
| 单元测试 | 423 passed / 0 failed |
| WOL 功能端到端接口测试 | 通过 |
| 边界条件（非法 / 重复 / 超限 MAC、开关联动） | 通过 |

---

## 许可证

沿用原项目的 GPL 许可证，详见 [LICENSE](LICENSE)。

需要说明的是，原项目仓库中 `LICENSE` 文件为 GPL-3.0，而 `Cargo.toml` 声明为 `GPL-2.0`，两者存在出入。本仓库保持与上游一致，未作改动。
