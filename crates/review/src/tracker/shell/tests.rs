use super::write_targets;

fn targets(command: &str) -> Vec<String> {
    write_targets(command)
}

#[test]
fn a_redirect_names_its_target() {
    assert_eq!(targets("echo hello > /tmp/out.txt"), ["/tmp/out.txt"]);
    assert_eq!(targets("echo hello >/tmp/out.txt"), ["/tmp/out.txt"]);
    assert_eq!(targets("echo hello >> /tmp/out.txt"), ["/tmp/out.txt"]);
    assert_eq!(targets("echo hello >| /tmp/out.txt"), ["/tmp/out.txt"]);
}

#[test]
fn a_redirect_of_one_stream_or_of_both_names_its_target() {
    assert_eq!(targets("make 2> /tmp/err.txt"), ["/tmp/err.txt"]);
    assert_eq!(targets("make 1>/tmp/out.txt"), ["/tmp/out.txt"]);
    assert_eq!(targets("make &> /tmp/all.txt"), ["/tmp/all.txt"]);
    assert_eq!(targets("make &>> /tmp/all.txt"), ["/tmp/all.txt"]);
    assert_eq!(targets("make >& /tmp/all.txt"), ["/tmp/all.txt"]);
}

#[test]
fn a_redirect_to_a_stream_or_a_device_names_no_file() {
    assert_eq!(targets("make 2>&1"), Vec::<String>::new());
    assert_eq!(targets("echo oops >&2"), Vec::<String>::new());
    assert_eq!(targets("exec 3>&-"), Vec::<String>::new());
    assert_eq!(targets("make > /dev/null 2>&1"), Vec::<String>::new());
    assert_eq!(targets("make 2>/dev/null"), Vec::<String>::new());
    assert_eq!(targets("echo x > /proc/self/fd/1"), Vec::<String>::new());
}

#[test]
fn a_redirect_that_reads_names_no_file() {
    assert_eq!(targets("sort < /tmp/in.txt"), Vec::<String>::new());
    assert_eq!(targets("cat <<< /tmp/in.txt"), Vec::<String>::new());
}

#[test]
fn quotes_and_escapes_belong_to_the_path() {
    assert_eq!(targets(r#"echo x > "/tmp/my dir/out.txt""#), ["/tmp/my dir/out.txt"]);
    assert_eq!(targets("echo x > '/tmp/my dir/out.txt'"), ["/tmp/my dir/out.txt"]);
    assert_eq!(targets(r"echo x > /tmp/my\ dir/out.txt"), ["/tmp/my dir/out.txt"]);
    assert_eq!(targets(r#"echo x > /tmp/"a b"/out.txt"#), ["/tmp/a b/out.txt"]);
}

#[test]
fn a_greater_than_inside_quotes_or_text_is_not_a_redirect() {
    assert_eq!(targets(r#"echo "a > /tmp/not.txt""#), Vec::<String>::new());
    assert_eq!(targets("echo 'a > /tmp/not.txt'"), Vec::<String>::new());
    assert_eq!(targets("echo a \\> /tmp/not.txt"), Vec::<String>::new());
}

#[test]
fn tee_names_every_file_it_is_given() {
    assert_eq!(targets("echo hi | tee /tmp/a.txt"), ["/tmp/a.txt"]);
    assert_eq!(targets("echo hi | tee -a /tmp/a.txt /tmp/b.txt"), ["/tmp/a.txt", "/tmp/b.txt"]);
    assert_eq!(targets("echo hi | sudo tee /tmp/a.txt > /dev/null"), Vec::<String>::new(), "sudo tee is not a plain tee");
}

#[test]
fn a_word_that_only_contains_tee_is_not_tee() {
    assert_eq!(targets("echo tee /tmp/a.txt"), Vec::<String>::new());
    assert_eq!(targets("committee /tmp/a.txt"), Vec::<String>::new());
}

#[test]
fn every_command_of_a_line_counts() {
    assert_eq!(targets("echo a > /tmp/a.txt && echo b >> /tmp/b.txt; echo c | tee /tmp/c.txt"), ["/tmp/a.txt", "/tmp/b.txt", "/tmp/c.txt"]);
    assert_eq!(targets("echo a > /tmp/a.txt\necho b > /tmp/b.txt"), ["/tmp/a.txt", "/tmp/b.txt"]);
}

#[test]
fn a_heredoc_names_its_target_and_its_body_names_nothing() {
    let command = "cat > /tmp/note.txt <<'EOF'\nline > /tmp/not-a-target.txt\nEOF\n";
    assert_eq!(targets(command), ["/tmp/note.txt"]);
    let dashed = "cat <<-END > /tmp/note.txt\n\tx > /tmp/nope.txt\n\tEND\necho done > /tmp/after.txt";
    assert_eq!(targets(dashed), ["/tmp/note.txt", "/tmp/after.txt"]);
}

#[test]
fn a_path_the_shell_would_expand_is_not_known() {
    assert_eq!(targets("echo x > $HOME/out.txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > ~/out.txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > /tmp/*.txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > /tmp/$(date).txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > `pwd`/out.txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > \"/tmp/${NAME}.txt\""), Vec::<String>::new());
}

#[test]
fn a_relative_path_is_left_to_git() {
    assert_eq!(targets("echo x > out.txt"), Vec::<String>::new());
    assert_eq!(targets("echo x > ../out.txt"), Vec::<String>::new());
}

#[test]
fn an_empty_or_odd_command_names_nothing() {
    assert_eq!(targets(""), Vec::<String>::new());
    assert_eq!(targets("   "), Vec::<String>::new());
    assert_eq!(targets("echo >"), Vec::<String>::new());
    assert_eq!(targets("echo \"unclosed > /tmp/x"), Vec::<String>::new());
}
