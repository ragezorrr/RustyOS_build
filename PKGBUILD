# Maintainer: ragezorrr <https://github.com/ragezorrr>
pkgname=aqua-compositor-git
_pkgname=aqua
pkgver=0.1.0.r1.g780f065
pkgrel=1
pkgdesc="A sleek macOS Sequoia-styled Wayland compositor written in 100% Rust"
arch=('x86_64')
url="https://github.com/ragezorrr/RustyOS"
license=('MIT')
depends=('seatd' 'libinput' 'mesa' 'glibc')
makedepends=('git' 'rust' 'cargo')
provides=('aqua')
conflicts=('aqua')
source=("git+https://github.com/ragezorrr/RustyOS.git")
sha256sums=('SKIP')

pkgver() {
    cd "RustyOS"
    printf "0.1.0.r%s.g%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
    cd "RustyOS"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "RustyOS"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-targets
}

package() {
    cd "RustyOS"
    install -Dm755 "target/release/aqua" "$pkgdir/usr/bin/aqua"
    install -Dm755 "aqua-update" "$pkgdir/usr/bin/aqua-update"
    install -Dm644 "aqua.desktop" "$pkgdir/usr/share/wayland-sessions/aqua.desktop"
}
