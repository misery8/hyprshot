pkgname=hyprshot
pkgver=0.2.1alpha
pkgrel=1
pkgdesc="Lightweight screenshot and annotation tool for Hyprland"
arch=('x86_64')
url="https://github.com/misery8/${pkgname}"
license=('GPL3')
depends=('gtk4' 'gdk-pixbuf2' 'glib2' 'cairo' 'grim' 'slurp')
makedepends=('cargo')
provides=("${pkgname}")
conflicts=("${pkgname}")
source=("${pkgname}::git+${url}.git")
sha256sums=('SKIP')

build() {
    cd "${pkgname}"
    export RUSTUP_TOOLCHAIN=stable
    cargo build --release --locked
}

package() {
    cd "${pkgname}"

    # Bin
    install -Dm755 "target/release/${pkgname}" -t "${pkgdir}/usr/bin/"

    # License
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"

    # Desktop-file
    install -Dm644 "resources/${pkgname}.desktop" \
        "${pkgdir}/usr/share/applications/${pkgname}.desktop"

    install -Dm644 "resources/icons/hyprshot.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${pkgname}.svg"
}