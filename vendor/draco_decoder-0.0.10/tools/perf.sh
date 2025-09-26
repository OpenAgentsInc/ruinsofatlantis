#!/bin/bash

# 检查是否提供了 EXAMPLE 参数
if [ -z "$1" ]; then
    echo "Usage: ./tools/perf.sh <example_name>"
    exit 1
fi

# 获取 EXAMPLE 参数
EXAMPLE=$1

# 执行 flamegraph 命令
cargo flamegraph --example $EXAMPLE --root -- --width 4000
