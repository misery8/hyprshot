pkgname=hyprshot
pkgver=0.1.0
pkgrel=1
pkgdesc="Lightweight screenshot and annotation tool for Hyprland"
arch=('x86_64')
url="https://github.com/misery8/${pkgname}"
license=('GPL3')
depends=('gtk4' 'gdk-pixbuf2' 'glib2' 'cairo' 'grim' 'slurp')
makedepends=('cargo')
provides=("${pkgname}")
conflicts=("${pkgname}")
source=("${pkgname}-${pkgver}.tar.gz::${url}/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('SKIP')

build() {
    cd "${pkgname}-${pkgver}"
    export RUSTUP_TOOLCHAIN=stable
    cargo build --release --locked
}

package() {
    cd "${pkgname}-${pkgver}"

    # Bin
    install -Dm755 "target/release/${pkgname}" -t "${pkgdir}/usr/bin/"

    # License
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"

    # Desktop-file
    isntall -Dm644 "resources/${pkgname}.desktop" \
        "${pkgdir}/usr/share/applications/${pkgname}.desktop"

    install -Dm644 "resources/icons/hyprshot.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${pkgname}.svg"
}