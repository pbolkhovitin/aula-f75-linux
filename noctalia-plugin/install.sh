#!/usr/bin/env bash
# Install the AULA F75 Noctalia widget against this checkout.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
binary="$root/target/release/f75-control"

if [[ ! -x "$binary" ]]; then
  echo "building f75-control…"
  cargo build --release --manifest-path "$root/Cargo.toml"
fi

mkdir -p "$HOME/.local/bin"
ln -sfn "$binary" "$HOME/.local/bin/f75-control"
echo "linked $HOME/.local/bin/f75-control -> $binary"

source_dir="$HOME/.local/share/noctalia-plugin-source-f75"
mkdir -p "$source_dir"
ln -sfn "$here" "$source_dir/aula-f75"

noctalia msg plugins source remove f75 >/dev/null 2>&1 || true
noctalia msg plugins source add f75 path "$source_dir"
noctalia msg plugins enable pbolkhovitin/aula-f75

cat <<'EOF'

Plugin installed. Add the widget to a bar:
  Settings → Bar → Add widget → AULA F75 Status
or append to ~/.local/state/noctalia/settings.toml:

  end = [ ..., "f75" ]

  [widget.f75]
  type = "pbolkhovitin/aula-f75:status"

Then restart Noctalia or run: noctalia msg config-reload
EOF
