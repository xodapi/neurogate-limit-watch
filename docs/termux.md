# Termux / Android Install Guide

nglimit runs natively on Android via Termux — no root required.

## Quick Install

```bash
# Install Rust toolchain
pkg install rust binutils
cargo install --path .

# Or build from source
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
cargo build --release
cp target/release/nglimit ~/.local/bin/
```

## Setup .env

```bash
mkdir -p ~/.config/nglimit
cat > ~/.config/nglimit/.env << 'EOF'
NEUROGATE_API_KEY=your_key_here
NEUROGATE_API_BASE=https://api.neurogate.space
EOF
```

## Usage

```bash
# One-shot check
nglimit --compact

# Live monitor (compact for narrow terminals)
nglimit --monitor --preset compact

# Widget-friendly output
nglimit --compact --json

# Notifications (requires Termux:API)
pkg install termux-api
nglimit --notify --watch 30
```

## Termux:Widget Integration

Create `~/.shortcuts/nglimit.sh`:

```bash
#!/data/data/com.termux/files/usr/bin/bash
nglimit --compact 2>/dev/null || echo "NG: err"
```

Make executable:
```bash
chmod +x ~/.shortcuts/nglimit.sh
```

Add the Termux:Widget widget to your home screen.

## Per-Window Thresholds

```bash
# Stricter on 5h window, relaxed on 30d
nglimit --monitor --threshold 5h=80:95,30d=85:98
```

## Troubleshooting

- **"cannot enable raw mode"**: Ensure you're running in Termux, not a GUI terminal emulator.
- **Missing notifications**: Install `termux-api` package and the Termux:API app from F-Droid.
- **Slow first run**: Rust compilation takes ~5 min on older devices. Use `--release` for optimized builds.
