use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use gtk4::{gio, glib};

#[derive(Clone, Copy)]
enum Shape {
    Outline,
    Fill,
    Palette,
}

const ICONS: [(&str, Shape); 10] = [
    ("diagonal-arrow-symbolic.svg", Shape::Outline),
    ("diagonal-arrow-active-symbolic.svg", Shape::Outline),
    ("rectangle-symbolic.svg", Shape::Outline),
    ("rectangle-active-symbolic.svg", Shape::Outline),
    ("undo-symbolic.svg", Shape::Outline),
    ("text-symbolic.svg", Shape::Fill),
    ("text-active-symbolic.svg", Shape::Fill),
    ("drop-water-symbolic.svg", Shape::Fill),
    ("drop-water-active-symbolic.svg", Shape::Fill),
    ("palette-symbolic.svg", Shape::Palette),
];

fn tag_attributes(svg: &str, element: &str) -> Result<BTreeMap<String, String>, String> {
    let start = format!("<{element} ");
    let open = svg
        .find(&start)
        .ok_or_else(|| format!("missing <{element}> element"))?;
    let tag = svg[open + start.len()..]
        .split_once('>')
        .ok_or_else(|| format!("unclosed <{element}> element"))?
        .0
        .trim_end_matches('/')
        .trim_end();

    let mut attributes = BTreeMap::new();
    let mut rest = tag;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        let (key, value) = rest
            .split_once('=')
            .ok_or_else(|| format!("malformed <{element}> attribute: {rest}"))?;
        let value = value
            .strip_prefix('"')
            .ok_or_else(|| format!("unquoted attribute {key}"))?;
        let (value, next) = value
            .split_once('"')
            .ok_or_else(|| format!("unterminated attribute {key}"))?;
        if attributes
            .insert(key.to_owned(), value.to_owned())
            .is_some()
        {
            return Err(format!("duplicate attribute {key}"));
        }
        rest = next;
    }
    Ok(attributes)
}

fn check_symbolic(svg: &str, shape: Shape) -> Result<(), String> {
    let mut tags = svg
        .split('<')
        .skip(1)
        .map(|segment| {
            segment
                .split(|ch: char| ch.is_whitespace() || ch == '>')
                .next()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    tags.retain(|tag| *tag != "?xml");
    tags.sort_unstable();
    if tags != ["/svg", "path", "svg"] {
        return Err(format!("unsupported SVG element(s): {tags:?}"));
    }

    let root = tag_attributes(svg, "svg")?;
    let allowed_root = ["xmlns", "viewBox", "width", "height"];
    if root.keys().any(|key| !allowed_root.contains(&key.as_str())) {
        return Err("unsupported SVG root attribute".into());
    }
    for (key, expected) in [
        ("xmlns", "http://www.w3.org/2000/svg"),
        ("viewBox", "0 0 24 24"),
        ("width", "24"),
        ("height", "24"),
    ] {
        if root.get(key).map(String::as_str) != Some(expected) {
            return Err(format!("root {key} must equal {expected}"));
        }
    }

    let attrs = tag_attributes(svg, "path")?;
    let allowed_path = [
        "d",
        "class",
        "fill",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "fill-rule",
    ];
    if attrs
        .keys()
        .any(|key| !allowed_path.contains(&key.as_str()))
    {
        return Err("unsupported GTK symbolic path attribute".into());
    }
    let value = |key| attrs.get(key).map(String::as_str);
    let d = value("d").ok_or("path missing d")?;
    if d.is_empty() {
        return Err("empty path geometry".into());
    }

    match shape {
        Shape::Outline => {
            if value("class") != Some("transparent-fill foreground-stroke")
                || value("fill") != Some("none")
                || value("stroke-width") != Some("1.9")
                || value("stroke").is_none()
            {
                return Err("outline lacks GTK foreground-stroke contract".into());
            }
        }
        Shape::Fill | Shape::Palette => {
            if value("class") != Some("foreground-fill") {
                return Err("filled figure lacks GTK foreground-fill contract".into());
            }
        }
    }

    if matches!(shape, Shape::Palette) {
        if value("fill-rule") != Some("evenodd")
            || d.chars().filter(|&ch| ch == 'M').count() != 5
            || ![
                "M 9.427 8.265",
                "M 13.162 6.937",
                "M 17.063 8.597",
                "M 8.846 12.249",
            ]
            .iter()
            .all(|hole| d.contains(hole))
        {
            return Err("palette must have four transparent compound-path cutouts".into());
        }
    } else if value("fill-rule").is_some() {
        return Err("unexpected fill rule".into());
    }

    Ok(())
}

#[test]
fn source_and_compiled_gresource_toolbar_icons_follow_gtk_symbolic_contract() {
    let resource_bytes = glib::Bytes::from_static(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/compiled.gresource"
    )));
    let resource = gio::Resource::from_data(&resource_bytes).expect("compiled GResource is valid");
    let manifest = include_str!("../resources/resources.gresource.xml");

    for (name, shape) in ICONS {
        let asset = format!("icons/symbolic/{name}");
        assert!(
            manifest.contains(&asset),
            "GResource manifest is missing {asset}"
        );

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(&asset);
        let source = fs::read_to_string(path).expect("symbolic source SVG exists");
        check_symbolic(&source, shape).unwrap_or_else(|error| panic!("{name}: {error}"));

        let themed_asset = format!("icons/scalable/actions/hyprshot-{name}");
        let themed_file = format!(
            r#"<file compressed="true" preprocess="xml-stripblanks" alias="{themed_asset}">{asset}</file>"#
        );
        assert!(
            manifest.contains(&themed_file),
            "{name}: missing GTK icon-theme GResource alias"
        );

        let resource_path = format!("/io/github/misery8/hyprshot/{asset}");
        let embedded = resource
            .lookup_data(&resource_path, gio::ResourceLookupFlags::NONE)
            .unwrap_or_else(|error| panic!("{resource_path}: {error}"));
        let embedded = std::str::from_utf8(embedded.as_ref()).expect("embedded SVG is UTF-8");
        let themed_path = format!("/io/github/misery8/hyprshot/{themed_asset}");
        let themed = resource
            .lookup_data(&themed_path, gio::ResourceLookupFlags::NONE)
            .unwrap_or_else(|error| panic!("{themed_path}: {error}"));
        assert_eq!(
            themed.as_ref(),
            embedded.as_bytes(),
            "{name}: GTK named icon and direct GResource versions differ"
        );
        check_symbolic(embedded, shape).unwrap_or_else(|error| panic!("embedded {name}: {error}"));

        assert_eq!(
            tag_attributes(&source, "path").unwrap(),
            tag_attributes(embedded, "path").unwrap(),
            "{name}: embedded geometry or symbolic colors are stale"
        );
    }
}

#[test]
fn old_stroke_only_and_masked_palette_markup_are_rejected() {
    let broken_arrow = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path d="M5 19 19 5" fill="none" stroke="#000000" stroke-width="2.25"/></svg>"##;
    assert!(check_symbolic(broken_arrow, Shape::Outline).is_err());

    let broken_palette = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><defs><mask id="holes"/></defs><path d="M12 1.75Z" mask="url(#holes)" fill="#000000"/></svg>"##;
    assert!(check_symbolic(broken_palette, Shape::Palette).is_err());
}
