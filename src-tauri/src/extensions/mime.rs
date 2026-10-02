//! Media types of bundle files, by the extension of the path (research R12). Anything unknown is
//! `application/octet-stream`, which a browser never runs as a script or renders as a page.

/// The `Content-Type` of a bundle file.
pub fn for_path(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// Whether a file of this path is shown as an image (icons in the launcher and the settings).
pub fn is_image(path: &str) -> bool {
    for_path(path).starts_with("image/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extensions_and_a_safe_default() {
        assert_eq!(for_path("index.html"), "text/html; charset=utf-8");
        assert_eq!(for_path("_nuxt/A.B.JS"), "text/javascript; charset=utf-8");
        assert_eq!(for_path("icon.svg"), "image/svg+xml");
        assert_eq!(for_path("README"), "application/octet-stream");
        assert_eq!(for_path("archive.tar.gz"), "application/octet-stream");
        assert!(is_image("a/b.png") && !is_image("a.js"));
    }
}
