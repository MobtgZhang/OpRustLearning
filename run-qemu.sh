#!/bin/bash
# LumenOS QEMU 运行器脚本
# 由 .cargo/config.toml 中的 runner 配置调用
# 用途：编译 builder（如需要），然后创建磁盘镜像并在 QEMU 中运行

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILDER_DIR="$SCRIPT_DIR/builder"

# 检测宿主机目标三元组
HOST_TARGET=$(rustc -vV | sed -n 's/host: //p')
BUILDER_BIN="$BUILDER_DIR/target/$HOST_TARGET/release/lumenos-builder"

# 如果 builder 不存在或源码更新，重新编译
if [ ! -f "$BUILDER_BIN" ] || [ "$BUILDER_DIR/src/main.rs" -nt "$BUILDER_BIN" ] || [ "$BUILDER_DIR/Cargo.toml" -nt "$BUILDER_BIN" ]; then
    echo "[builder] 编译磁盘镜像构建工具..."
    (cd "$BUILDER_DIR" && cargo build --release --target "$HOST_TARGET" 2>&1 | tail -5)
fi

# 运行 builder（传递所有参数）
exec "$BUILDER_BIN" "$@"
