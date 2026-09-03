# 清枫速记

Windows 桌面速记工具，Tauri 写的。笔记存本地 `.md`，离线用。

## 下载

- 绿色版：`QFNOTE-绿色版/qfnote.exe`，双击就跑，随便放哪。
- 安装版：`QFNOTE-安装包/清枫速记_2.7.1_x64_zh-CN.msi`，双击装，进开始菜单、关联 `.md`。

## 热键

Ctrl+1~5 切标签页 · Ctrl+U 网页转笔记 · Tab 迷你/主窗口切换 · F11 截图插入 · Esc 关

## 构建

```
npm install
npm run build
```

要装 Rust（stable, MSVC）和 Node LTS，系统得有 WebView2。

## 笔记在哪

`文档\qfnote\`，一个笔记一个 `.md` 文件。

## 许可证

Apache-2.0，版权 2026 清枫不识客。看 [LICENSE](LICENSE)。
