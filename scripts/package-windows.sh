#!/usr/bin/env bash
# Runquiry Windows 打包（模块 08 D2 干净 runner）：
#   1. cargo about 重新生成第三方许可证清单（保持与 Cargo.lock 同步）
#   2. cargo build --release -p runquiry-app --locked
#   3. cargo packager 生成 MSI（无签名）
#   4. 组装便携版 zip（exe + 许可证 + NOTICE + LICENSE + 说明）
#   5. 生成 dist/SHA256SUMS 并断言 Cargo.lock 未被改动
# 前置工具：cargo-about（--features cli）、cargo-packager（均 cargo install --locked）。
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> 生成第三方许可证清单"
cargo about generate about.hbs --output-file docs/third-party-licenses.md

echo "==> 构建 release（--locked）"
cargo build --release -p runquiry-app --locked

echo "==> 生成 MSI"
# 显式 -c：workspace 根的 Packager.toml 不参与 CLI 默认探测。
cargo packager -c Packager.toml --formats wix

echo "==> 组装便携版"
portable_dir="dist/runquiry-portable"
rm -rf "$portable_dir"
mkdir -p "$portable_dir"
cp target/release/runquiry.exe "$portable_dir/"
cp docs/third-party-licenses.md "$portable_dir/"
cp NOTICE "$portable_dir/"
cp LICENSE "$portable_dir/"
cat > "$portable_dir/README-portable.txt" <<'EOF'
Runquiry 便携版（Portable）
===========================

本地进程/端口/容器/文件锁调查桌面应用。
本目录版本无需安装：直接运行 runquiry.exe 即可。

- 本包无数字签名：来源仅为 Runquiry 项目官方发布渠道，请自行校验 SHA256SUMS。
- 运行期完全本地：无遥测、云服务、自动更新或后台网络请求。
- 设置保存在 %APPDATA%\runquiry\settings.json；删除该目录即重置。
- 许可证：GPL-3.0-or-later（LICENSE）；第三方依赖清单见 third-party-licenses.md；
  witr 参考实现归属见 NOTICE。

Runquiry portable edition. Run runquiry.exe directly; no installer required.
Unsigned build — verify SHA256SUMS. Fully local at runtime, no telemetry.
EOF
portable_zip="dist/runquiry-portable-windows-x64.zip"
rm -f "$portable_zip"
powershell -NoProfile -Command "Compress-Archive -Path 'dist/runquiry-portable/*' -DestinationPath '$portable_zip' -Force"

echo "==> 生成 SHA-256 清单"
: > dist/SHA256SUMS
for f in dist/*.msi "$portable_zip"; do
    [ -e "$f" ] || continue
    sha256sum "$f" >> dist/SHA256SUMS
done

echo "==> 断言 Cargo.lock 未漂移"
git diff --exit-code -- Cargo.lock || {
    echo "错误：打包过程改动了 Cargo.lock（必须保持 --locked）" >&2
    exit 1
}

echo "==> 完成："
ls -la dist/
cat dist/SHA256SUMS
