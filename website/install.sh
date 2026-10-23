#!/bin/sh
# Install the latest KbWrite release on macOS or Linux.
set -eu

REPO="zetahiveco/kbwrite"
API="https://api.github.com/repos/${REPO}/releases/latest"

os=$(uname -s)
arch=$(uname -m)

json=$(curl -fsSL \
  -H "Accept: application/vnd.github+json" \
  -H "User-Agent: kbwrite-install" \
  "$API") || {
  echo "Could not reach GitHub releases." >&2
  exit 1
}

urls=$(printf '%s\n' "$json" | grep -o 'https://github.com/zetahiveco/kbwrite/releases/download/[^"]*')

if [ -z "$urls" ]; then
  echo "No release files found." >&2
  exit 1
fi

# First download URL whose filename matches the pattern. Avoids grep|head
# exiting early under `set -e`.
first_match() {
  pattern=$1
  printf '%s\n' "$urls" | while IFS= read -r line; do
    name=${line##*/}
    if printf '%s\n' "$name" | grep -Eqi "$pattern"; then
      printf '%s\n' "$line"
      break
    fi
  done
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

download() {
  echo "Downloading $(basename "$1")..."
  curl -fL --progress-bar -o "$2" "$1"
}

case "$os" in
  Darwin)
    case "$arch" in
      arm64|aarch64) pattern='aarch64\.app\.tar\.gz$' ;;
      x86_64) pattern='x64\.app\.tar\.gz$' ;;
      *)
        echo "Unsupported Mac architecture: $arch" >&2
        exit 1
        ;;
    esac
    url=$(first_match "$pattern" || true)
    if [ -z "$url" ]; then
      echo "No macOS build found for $arch." >&2
      exit 1
    fi
    download "$url" "$tmp/app.tar.gz"
    tar -xzf "$tmp/app.tar.gz" -C "$tmp"
    app=""
    for candidate in "$tmp"/*.app "$tmp"/*/*.app; do
      if [ -d "$candidate" ]; then
        app=$candidate
        break
      fi
    done
    if [ -z "$app" ]; then
      echo "The archive did not contain an app." >&2
      exit 1
    fi
    if [ -w /Applications ]; then
      dest="/Applications"
    else
      dest="${HOME}/Applications"
      mkdir -p "$dest"
    fi
    name=$(basename "$app")
    rm -rf "${dest}/${name}"
    cp -R "$app" "$dest/"
    xattr -dr com.apple.quarantine "${dest}/${name}" 2>/dev/null || true
    echo "Installed to ${dest}/${name}"
    ;;

  Linux)
    case "$arch" in
      x86_64|amd64) archpat='amd64|x86_64|x64' ;;
      aarch64|arm64) archpat='aarch64|arm64' ;;
      *)
        echo "Unsupported Linux architecture: $arch" >&2
        exit 1
        ;;
    esac
    url=$(first_match "appimage.*(${archpat})|(${archpat}).*appimage" || true)
    kind="appimage"
    if [ -z "$url" ]; then
      url=$(first_match "deb.*(${archpat})|(${archpat}).*\\.deb" || true)
      kind="deb"
    fi
    if [ -z "$url" ]; then
      echo "No Linux build found for $arch." >&2
      printf '%s\n' "$urls" >&2
      exit 1
    fi
    file="${tmp}/$(basename "$url")"
    download "$url" "$file"
    if [ "$kind" = "appimage" ]; then
      mkdir -p "${HOME}/.local/bin"
      install -m 755 "$file" "${HOME}/.local/bin/kbwrite"
      echo "Installed to ${HOME}/.local/bin/kbwrite"
      case ":${PATH}:" in
        *":${HOME}/.local/bin:"*) ;;
        *) echo "Add \$HOME/.local/bin to your PATH, then run: kbwrite" ;;
      esac
    else
      echo "Installing the package. sudo may ask for your password."
      sudo dpkg -i "$file" || sudo apt-get install -f -y
      echo "Installed. Run: kbwrite"
    fi
    ;;

  *)
    echo "This installer is for macOS and Linux." >&2
    echo "On Windows, in PowerShell, run:" >&2
    echo "  irm https://kbwrite.ink/install.ps1 | iex" >&2
    exit 1
    ;;
esac
