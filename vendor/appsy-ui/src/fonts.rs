//! Embedded Geist faces → `@font-face` rules. Default path: base64 `data:`
//! URIs (latin / latin-ext / cyrillic / cyrillic-ext / vietnamese × normal +
//! italic), so the stylesheet is self-contained with nothing to serve.
//! Registered family names are `"Geist"`/`"Geist Mono"` — matching
//! `tokens.rs` exactly, which is the approved fix for the upstream bug where
//! tokens requested names no `@font-face` registered. The binaries are the
//! same fontsource variable-weight woff2 the reference ships, so glyph
//! metrics (and therefore intrinsic text widths) match the reference.
//!
//! The `font-files` feature is the alternative for consumers that serve a
//! `/fonts/` route: `@font-face` sources become `url(/fonts/{name})` and
//! [`files`] exposes the embedded woff2 to serve (better LCP/caching). Both
//! paths are unit-tested per the invariant.

struct Face {
    family: &'static str,
    /// File name under the consumer's `/fonts/` route (`font-files` path).
    #[cfg_attr(not(feature = "font-files"), allow(dead_code))]
    file: &'static str,
    bytes: &'static [u8],
    unicode_range: &'static str,
    /// `"normal"` or `"italic"`.
    style: &'static str,
}

/// Unicode ranges verbatim from the fontsource css the reference imports.
const LATIN: &str = "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD";
const LATIN_EXT: &str = "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF";
const CYRILLIC: &str = "U+0301,U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116";
const CYRILLIC_EXT: &str = "U+0460-052F,U+1C80-1C8A,U+20B4,U+2DE0-2DFF,U+A640-A69F,U+FE2E-FE2F";
const VIETNAMESE: &str = "U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB";

const FACES: &[Face] = &[
    // Geist — normal
    Face {
        family: "Geist",
        file: "geist-latin-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-latin-wght-normal.woff2"),
        unicode_range: LATIN,
        style: "normal",
    },
    Face {
        family: "Geist",
        file: "geist-latin-ext-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-latin-ext-wght-normal.woff2"),
        unicode_range: LATIN_EXT,
        style: "normal",
    },
    Face {
        family: "Geist",
        file: "geist-cyrillic-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-cyrillic-wght-normal.woff2"),
        unicode_range: CYRILLIC,
        style: "normal",
    },
    Face {
        family: "Geist",
        file: "geist-cyrillic-ext-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-cyrillic-ext-wght-normal.woff2"),
        unicode_range: CYRILLIC_EXT,
        style: "normal",
    },
    Face {
        family: "Geist",
        file: "geist-vietnamese-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-vietnamese-wght-normal.woff2"),
        unicode_range: VIETNAMESE,
        style: "normal",
    },
    // Geist — italic
    Face {
        family: "Geist",
        file: "geist-latin-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-latin-wght-italic.woff2"),
        unicode_range: LATIN,
        style: "italic",
    },
    Face {
        family: "Geist",
        file: "geist-latin-ext-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-latin-ext-wght-italic.woff2"),
        unicode_range: LATIN_EXT,
        style: "italic",
    },
    Face {
        family: "Geist",
        file: "geist-cyrillic-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-cyrillic-wght-italic.woff2"),
        unicode_range: CYRILLIC,
        style: "italic",
    },
    Face {
        family: "Geist",
        file: "geist-cyrillic-ext-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-cyrillic-ext-wght-italic.woff2"),
        unicode_range: CYRILLIC_EXT,
        style: "italic",
    },
    Face {
        family: "Geist",
        file: "geist-vietnamese-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-vietnamese-wght-italic.woff2"),
        unicode_range: VIETNAMESE,
        style: "italic",
    },
    // Geist Mono — normal
    Face {
        family: "Geist Mono",
        file: "geist-mono-latin-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-mono-latin-wght-normal.woff2"),
        unicode_range: LATIN,
        style: "normal",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-latin-ext-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-mono-latin-ext-wght-normal.woff2"),
        unicode_range: LATIN_EXT,
        style: "normal",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-cyrillic-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-mono-cyrillic-wght-normal.woff2"),
        unicode_range: CYRILLIC,
        style: "normal",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-cyrillic-ext-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-mono-cyrillic-ext-wght-normal.woff2"),
        unicode_range: CYRILLIC_EXT,
        style: "normal",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-vietnamese-wght-normal.woff2",
        bytes: include_bytes!("../assets/geist-mono-vietnamese-wght-normal.woff2"),
        unicode_range: VIETNAMESE,
        style: "normal",
    },
    // Geist Mono — italic
    Face {
        family: "Geist Mono",
        file: "geist-mono-latin-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-mono-latin-wght-italic.woff2"),
        unicode_range: LATIN,
        style: "italic",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-latin-ext-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-mono-latin-ext-wght-italic.woff2"),
        unicode_range: LATIN_EXT,
        style: "italic",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-cyrillic-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-mono-cyrillic-wght-italic.woff2"),
        unicode_range: CYRILLIC,
        style: "italic",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-cyrillic-ext-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-mono-cyrillic-ext-wght-italic.woff2"),
        unicode_range: CYRILLIC_EXT,
        style: "italic",
    },
    Face {
        family: "Geist Mono",
        file: "geist-mono-vietnamese-wght-italic.woff2",
        bytes: include_bytes!("../assets/geist-mono-vietnamese-wght-italic.woff2"),
        unicode_range: VIETNAMESE,
        style: "italic",
    },
];

/// One embedded woff2 for the consumer's `/fonts/` route (`font-files` path).
/// The stylesheet references it as `url(/fonts/{name})`; serve `bytes` there
/// with `Content-Type: font/woff2` and a long cache lifetime.
#[cfg(feature = "font-files")]
pub struct FontFile {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

/// The woff2 binaries the `font-files` stylesheet references — everything a
/// consumer must serve, nothing else.
#[cfg(feature = "font-files")]
pub fn files() -> Vec<FontFile> {
    FACES.iter().map(|f| FontFile { name: f.file, bytes: f.bytes }).collect()
}

/// `@font-face` rules. Default: base64 `data:` sources, self-contained.
/// `font-files`: `url(/fonts/{name})` sources served by the consumer.
pub fn css() -> String {
    let mut out = String::new();
    for face in FACES {
        #[cfg(feature = "font-files")]
        let src = format!("/fonts/{}", face.file);
        #[cfg(not(feature = "font-files"))]
        let src = format!("data:font/woff2;base64,{}", base64(face.bytes));
        out.push_str(&format!(
            "@font-face{{font-family:\"{}\";font-style:{};font-display:swap;\
             font-weight:100 900;src:url({src}) \
             format(\"woff2-variations\");unicode-range:{};}}",
            face.family, face.style, face.unicode_range
        ));
    }
    out
}

/// Minimal standard base64 — written in place of a dependency, per the
/// minimal-deps invariant. Only the default `data:` path encodes.
#[cfg(not(feature = "font-files"))]
fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;

    #[cfg(not(feature = "font-files"))]
    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }

    /// Both directions of the font-name contract: every token family has a
    /// face, and no face registers a family the tokens never reference.
    #[test]
    fn faces_and_tokens_agree_on_family_names() {
        for family in tokens::FONT_FAMILIES {
            assert!(
                FACES.iter().any(|f| f.family == *family),
                "token family {family:?} has no @font-face"
            );
        }
        for face in FACES {
            assert!(
                tokens::FONT_FAMILIES.contains(&face.family),
                "@font-face registers {:?}, which no token references",
                face.family
            );
        }
    }

    #[test]
    fn faces_cover_normal_and_italic() {
        assert!(FACES.iter().any(|f| f.style == "normal"));
        assert!(FACES.iter().any(|f| f.style == "italic"));
        assert!(FACES.len() >= 20);
        for face in FACES {
            assert!(
                face.style == "normal" || face.style == "italic",
                "unexpected style {:?}",
                face.style
            );
        }
    }

    #[cfg(not(feature = "font-files"))]
    #[test]
    fn css_embeds_every_face_as_data_uri() {
        let css = css();
        assert_eq!(css.matches("@font-face").count(), FACES.len());
        assert_eq!(css.matches("url(data:font/woff2;base64,").count(), FACES.len());
        // woff2 magic number `wOF2` encodes to `d09G`.
        assert_eq!(css.matches("base64,d09G").count(), FACES.len());
        assert!(css.contains("font-style:italic"));
        assert!(css.contains("font-style:normal"));
    }

    /// `font-files` path: every face sources a served file, no data: URIs,
    /// and [`files`] exposes exactly the referenced set.
    #[cfg(feature = "font-files")]
    #[test]
    fn css_references_served_files() {
        let css = css();
        assert_eq!(css.matches("@font-face").count(), FACES.len());
        assert!(!css.contains("data:"), "font-files path must not embed data URIs");
        assert!(css.contains("font-style:italic"));
        for file in files() {
            assert!(
                css.contains(&format!("url(/fonts/{})", file.name)),
                "no src for served file {}",
                file.name
            );
        }
    }

    /// The served set is complete, unique, and genuinely woff2.
    #[cfg(feature = "font-files")]
    #[test]
    fn files_are_woff2_and_unique() {
        let served = files();
        assert_eq!(served.len(), FACES.len());
        let mut names: Vec<_> = served.iter().map(|f| f.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FACES.len(), "duplicate served file name");
        for file in served {
            assert_eq!(&file.bytes[..4], b"wOF2", "{} is not woff2", file.name);
            assert!(file.name.ends_with(".woff2"));
        }
    }
}
