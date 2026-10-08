use super::*;

#[test]
fn a_complete_head_is_parsed_with_its_range() {
    let head =
        parse_head(b"GET /abc HTTP/1.1\r\nHost: 127.0.0.1\r\nrange: bytes=0-9\r\n\r\n").unwrap();
    assert_eq!(
        head,
        Head {
            method: "GET".to_owned(),
            path: "/abc".to_owned(),
            range: Some("bytes=0-9".to_owned()),
        }
    );
}

#[test]
fn an_incomplete_or_foreign_head_is_none() {
    assert_eq!(parse_head(b"GET /abc HTTP/1.1\r\nHost: x\r\n"), None);
    assert_eq!(parse_head(b"hello\r\n\r\n"), None);
    assert_eq!(parse_head(b"GET /abc SPDY\r\n\r\n"), None);
}

#[test]
fn a_head_without_range() {
    assert_eq!(parse_head(b"HEAD /t HTTP/1.1\r\n\r\n").unwrap().range, None);
}

#[test]
fn only_a_single_path_segment_is_a_token() {
    assert_eq!(token_of("/abc"), Some("abc"));
    assert_eq!(token_of("/abc?x=1"), Some("abc"));
    assert_eq!(token_of("/"), None);
    assert_eq!(token_of("/a/b"), None);
    assert_eq!(token_of("abc"), None);
}

#[test]
fn every_answer_carries_the_cors_headers_and_no_encoding() {
    for head in [
        empty(404, "Not Found"),
        unsatisfiable(10),
        body("video/mp4", 10, None),
        body("video/mp4", 5, Some((0, 4, 10))),
    ] {
        assert!(
            head.contains("Access-Control-Allow-Origin: *\r\n"),
            "{head}"
        );
        assert!(
            head.contains(
                "Access-Control-Expose-Headers: Accept-Ranges, Content-Range, Content-Length\r\n"
            ),
            "{head}"
        );
        assert!(head.contains("Cache-Control: no-store\r\n"), "{head}");
        assert!(
            !head.to_ascii_lowercase().contains("content-encoding"),
            "{head}"
        );
        assert!(head.ends_with("\r\n\r\n"), "{head}");
    }
}

#[test]
fn a_part_names_its_range_and_the_whole_does_not() {
    let part = body("video/mp4", 5, Some((10, 14, 100)));
    assert!(part.starts_with("HTTP/1.1 206 Partial Content\r\n"));
    assert!(part.contains("Content-Range: bytes 10-14/100\r\n"));
    assert!(part.contains("Content-Length: 5\r\n"));
    assert!(part.contains("Accept-Ranges: bytes\r\n"));
    let whole = body("audio/mpeg", 100, None);
    assert!(whole.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(!whole.contains("\r\nContent-Range:"));
}

#[test]
fn an_unsatisfiable_range_tells_the_size() {
    let head = unsatisfiable(1234);
    assert!(head.starts_with("HTTP/1.1 416 Range Not Satisfiable\r\n"));
    assert!(head.contains("Content-Range: bytes */1234\r\n"));
}
