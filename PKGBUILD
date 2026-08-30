pkgname=hyprshot
pkgver=0.4.0
pkgrel=1
pkgdesc="Lightweight screenshot and annotation tool for Hyprland (Rust version)"
arch=('x86_64')
url="https://github.com/misery8/${pkgname}"
license=('GPL3')
depends=('gtk4' 'gdk-pixbuf2' 'glib2' 'cairo' 'grim' 'gtk4-layer-shell')
makedepends=('cargo' 'git')
source=("${pkgname}-${pkgver}::git+${url}.git#tag=v${pkgver}")
sha256sums=('SKIP')

prepare() {
    cd "${pkgname}-${pkgver}"
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "${pkgname}-${pkgver}"
    export RUSTFLAGS="-C opt-level=3 -C debuginfo=0"
    cargo build --release --locked --all-targets
}

package() {
    cd "${pkgname}-${pkgver}"

    # Bin4
    install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
    install -Dm755 "target/release/clipboard" "${pkgdir}/usr/lib/${pkgname}/clipboard"

    # License
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"

    # Desktop-file
    install -Dm644 "resources/${pkgname}.desktop" \
        "${pkgdir}/usr/share/applications/${pkgname}.desktop"

    install -Dm644 "resources/icons/hyprshot.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${pkgname}.svg"
}