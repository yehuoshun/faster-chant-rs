#!/bin/bash
# GitHub 推送脚本：拉取走代理，推送直连
# 用法: ./push.sh "commit message"

set -e
cd "$(dirname "$0")"

# 配置代理（克隆/拉取走 gh-proxy，推送直连）
git config url."https://gh-proxy.com/https://github.com/".insteadOf "https://github.com/"
git config url."https://github.com/".pushInsteadOf "https://gh-proxy.com/https://github.com/"
git config http.lowSpeedLimit 1000
git config http.lowSpeedTime 10

MSG="${1:-update}"

# 拉取最新
git pull origin main 2>/dev/null || echo "拉取跳过（可能已是最新）"

# 提交
git add -A
if git diff --cached --quiet; then
    echo "无变更，跳过提交"
    exit 0
fi

git commit -m "$MSG"

# 推送（直连 GitHub，重试最多 5 次）
for i in 1 2 3 4 5; do
    echo "推送尝试 $i/5..."
    if timeout 60 git push origin main 2>&1; then
        echo "✅ 推送成功"
        exit 0
    fi
    echo "推送失败，${i}s 后重试..."
    sleep 2
done

echo "❌ 推送失败，请手动推送"
exit 1