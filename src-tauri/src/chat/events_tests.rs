use super::*;

#[test]
fn strip_leaked_tool_call_markup_removes_a_qwen_style_tag() {
    let leaked = "<tool_call>\n{\"name\": \"run_command\", \"arguments\": {\"command\": \"ls -a ~/Downloads\"}}\n</tool_call>";
    assert_eq!(strip_leaked_tool_call_markup(leaked), "");
}

#[test]
fn strip_leaked_tool_call_markup_keeps_prose_before_the_tag() {
    let leaked =
        "Let me check that.\n<tool_call>{\"name\": \"echo\", \"arguments\": {}}</tool_call>";
    assert_eq!(
        strip_leaked_tool_call_markup(leaked),
        "Let me check that.\n"
    );
}

#[test]
fn strip_leaked_tool_call_markup_removes_multiple_tags() {
    let leaked = "<tool_call>{\"name\": \"a\"}</tool_call><tool_call>{\"name\": \"b\"}</tool_call>";
    assert_eq!(strip_leaked_tool_call_markup(leaked), "");
}

#[test]
fn strip_leaked_tool_call_markup_is_a_no_op_without_a_tag() {
    assert_eq!(
        strip_leaked_tool_call_markup("just a normal reply"),
        "just a normal reply"
    );
}
