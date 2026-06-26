# Termux / Android Install Guide

vimit runs natively on Android via Termux — no root required.

## Quick Install

```bash
# Install Rust toolchain
pkg install rust binutils
cargo install --path .

# Or build from source
git clone https://github.com/xodapi/vimit.git
cd vimit
cargo build --release
cp target/release/vimit ~/.local/bin/
```

## Setup .env

```bash
mkdir -p ~/.config/vimit
cat > ~/.config/vimit/.env << 'EOF'
VIBEMODE_API_KEY=your_key_here
VIBEMODE_API_BASE=https://r-api.vibemod.pro
EOF
```

## Usage

```bash
# One-shot check
vimit --compact

# Live monitor (compact for narrow terminals)
vimit --monitor --preset compact

# Widget-friendly output
vimit --compact --json

# Notifications (requires Termux:API)
pkg install termux-api
vimit --notify --watch 30
```

## Termux:Widget Integration

Create `~/.shortcuts/vimit.sh`:

```bash
#!/data/data/com.termux/files/usr/bin/bash
vimit --compact 2>/dev/null || echo "NG: err"
```

Make executable:
```bash
chmod +x ~/.shortcuts/vimit.sh
```

Add the Termux:Widget widget to your home screen.

## Per-Window Thresholds

```bash
# Stricter on 5h window, relaxed on 30d
vimit --monitor --threshold 5h=80:95,30d=85:98
```

## Troubleshooting

- **"cannot enable raw mode"**: Ensure you're running in Termux, not a GUI terminal emulator.
- **Missing notifications**: Install `termux-api` package and the Termux:API app from F-Droid.
- **Slow first run**: Rust compilation takes ~5 min on older devices. Use `--release` for optimized builds.
