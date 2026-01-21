# AMM - Anti-idle Mouse Mover

Windows 防空闲工具，在用户空闲时自动微移鼠标，防止系统锁屏/休眠、IM 状态变为离开。

## 特性

- 托盘运行，无窗口
- 空闲检测，用户操作时立即停止
- 多种移动模式：ping_pong / micro_jitter / random_walk_box
- 全屏应用自动暂停
- 极低资源占用（CPU < 0.1%, 内存 < 10MB）

## 使用

1. 运行 `amm.exe`
2. 托盘右键菜单控制：暂停/继续/退出
3. 配置文件 `amm.toml`（可选）

## 配置

```toml
idle_threshold_ms = 120000  # 空闲阈值（毫秒）
interval_ms = 30000         # 注入间隔
jitter_ms = 5000            # 间隔抖动
move_pattern = "ping_pong"  # 移动模式
pause_on_fullscreen = true  # 全屏暂停
```

## 构建

```bash
cargo build --release
```

## License

MIT
