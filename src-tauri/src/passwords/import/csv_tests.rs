use super::*;

#[test]
fn a_line_break_inside_quotes_stays_in_the_cell() {
    let table = parse(b"name,notes\r\n\"Mail\",\"line one\r\nline two\"\r\n").unwrap();
    assert_eq!(table.rows.len(), 1);
    assert_eq!(
        table.cell(&table.rows[0], "notes"),
        Some("line one\r\nline two")
    );
}

#[test]
fn doubled_quotes_and_a_bom_are_read() {
    let table = parse("\u{feff}Name,Notes\n\"say \"\"hi\"\"\",x\n".as_bytes()).unwrap();
    assert_eq!(table.headers, vec!["name", "notes"]);
    assert_eq!(table.cell(&table.rows[0], "name"), Some("say \"hi\""));
}

#[test]
fn a_short_row_reads_as_empty_cells_and_an_unknown_column_as_none() {
    let table = parse(b"a,b,c\n1,2\n").unwrap();
    assert_eq!(table.cell(&table.rows[0], "c"), Some(""));
    assert_eq!(table.cell(&table.rows[0], "d"), None);
}

#[test]
fn blank_lines_are_skipped_and_text_that_is_not_utf8_is_corrupt() {
    let table = parse(b"a,b\n1,2\n\n3,4\n").unwrap();
    assert_eq!(table.rows.len(), 2);
    assert!(parse(&[0xff, 0xfe, 0x00]).is_err());
    assert!(parse(b"").is_err());
}
