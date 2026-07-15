#!/bin/bash
# GitHub 推送脚本：全部走代理，一条命令搞定
# 用法: ./push.sh "commit message"

set -e
cd "$(dirname "$0")"

# 从环境变量读 token
TOKEN="${GITHUB_PAT_CLASSIC:-}"
if [ -z "$TOKEN" ]; then
    echo "❌ 请设置 GITHUB_PAT_CLASSIC 环境变量"
    exit 1
fi

MSG="${1:-update}"

# 配置代理
git config url."https://gh-proxy.com/https://github.com/".insteadOf "https://github.com/"
git config http.lowSpeedLimit 1000
git config http.lowSpeedTime 10

# 远程地址（token 在 URL 中）
git remote set-url origin "https://yehuoshun:${TOKEN}@gh-proxy.com/https://github.com/yehuoshun/faster-chant-rs.git"

# 拉取最新
git pull origin main 2>/dev/null || echo "拉取跳过"

# 提交
git add -A
if git diff --cached --quiet; then
    echo "无变更"
    exit 0
fi

git commit -m "$MSG"

# 推送
timeout 90 git push origin main && echo "✅ 推送成功" || echo "❌ 推送失败"