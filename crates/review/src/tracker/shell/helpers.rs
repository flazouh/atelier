use super::types::Token;

/// Where a redirect is not a file: a device, or the kernel's own files.
const NOT_FILES: [&str; 3] = ["/dev/", "/proc/", "/sys/"];

/// The absolute paths `command` writes to by name, in the order it names them.
pub(in super::super) fn write_targets(command: &str) -> Vec<String> {
    let Some(tokens) = lex(&without_heredoc_bodies(command)) else { return Vec::new() };
    let mut targets = Vec::new();
    let mut at_start = true;
    let mut in_tee = false;
    let mut tokens = tokens.iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            Token::Op(op) => match op.as_str() {
                ">" | ">>" | ">|" | "&>" | "&>>" => targets.extend(file_named(tokens.next())),
                // `>&2` and `>&-` copy or close a stream; `>& file` writes both streams to a file.
                ">&" => match tokens.next() {
                    Some(Token::Word { text, .. }) if text == "-" || text.chars().all(|c| c.is_ascii_digit()) => {}
                    other => targets.extend(file_named(other)),
                },
                // A redirect that reads, and a heredoc's delimiter: the next word is not a file written.
                "<" | "<<" | "<<-" | "<<<" | "<&" => {
                    tokens.next();
                }
                _ => {
                    at_start = true;
                    in_tee = false;
                }
            },
            Token::Word { text, .. } if in_tee => {
                if !text.starts_with('-') {
                    targets.extend(file_named(Some(token)));
                }
            }
            Token::Word { text, .. } if at_start => {
                in_tee = text == "tee";
                at_start = false;
            }
            Token::Word { .. } => {}
        }
    }
    targets
}

/// The word's text when it names a file the shell would write as it stands.
fn file_named(token: Option<&Token>) -> Option<String> {
    let Some(Token::Word { text, expands: false }) = token else { return None };
    (text.starts_with('/') && !text.ends_with('/') && !NOT_FILES.iter().any(|folder| text.starts_with(folder))).then(|| text.clone())
}

/// The command without the text of its heredocs: a body is data, and `a > b` in it writes nothing.
fn without_heredoc_bodies(command: &str) -> String {
    let mut kept = Vec::new();
    let mut ending: Option<(String, bool)> = None;
    for line in command.lines() {
        if let Some((delimiter, dashed)) = &ending {
            let candidate = if *dashed { line.trim_start_matches('\t') } else { line };
            if candidate == delimiter {
                ending = None;
            }
            continue;
        }
        ending = heredoc_delimiter(line);
        kept.push(line);
    }
    kept.join("\n")
}

/// The delimiter a line's heredoc ends at, and whether leading tabs may precede it (`<<-`).
fn heredoc_delimiter(line: &str) -> Option<(String, bool)> {
    let mut from = 0;
    while let Some(found) = line[from..].find("<<") {
        let at = from + found;
        if line[at..].starts_with("<<<") {
            from = at + 3;
            continue;
        }
        let rest = &line[at + 2..];
        let (dashed, rest) = rest.strip_prefix('-').map_or((false, rest), |rest| (true, rest));
        let word: String = rest
            .trim_start()
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')'))
            .filter(|c| !matches!(c, '\'' | '"' | '\\'))
            .collect();
        return (!word.is_empty()).then_some((word, dashed));
    }
    None
}

/// Cuts a shell line into words and operators. `None` when a quote never closes: the line is not whole, so it names nothing.
fn lex(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let (mut word, mut started, mut expands) = (String::new(), false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => flush(&mut tokens, &mut word, &mut started, &mut expands),
            '\n' | ';' | '|' | '(' | ')' => {
                flush(&mut tokens, &mut word, &mut started, &mut expands);
                let mut op = c.to_string();
                if c == '|' && matches!(chars.get(i + 1), Some('|' | '&')) {
                    op.push(chars[i + 1]);
                    i += 1;
                }
                tokens.push(Token::Op(op));
            }
            '&' => {
                flush(&mut tokens, &mut word, &mut started, &mut expands);
                let rest: String = chars[i..].iter().take(3).collect();
                let op = ["&>>", "&>", "&&"].into_iter().find(|op| rest.starts_with(op)).unwrap_or("&");
                i += op.len() - 1;
                tokens.push(Token::Op(op.to_string()));
            }
            '>' | '<' => {
                // A number right before it names the stream (`2>`), and is not a word of its own.
                if started && word.chars().all(|c| c.is_ascii_digit()) {
                    word.clear();
                    (started, expands) = (false, false);
                } else {
                    flush(&mut tokens, &mut word, &mut started, &mut expands);
                }
                let rest: String = chars[i..].iter().take(3).collect();
                let ops: &[&str] = if c == '>' { &[">>", ">|", ">&"] } else { &["<<<", "<<-", "<<", "<&"] };
                let op = ops.iter().copied().find(|op| rest.starts_with(op)).unwrap_or(if c == '>' { ">" } else { "<" });
                i += op.len() - 1;
                tokens.push(Token::Op(op.to_string()));
            }
            '\'' => {
                started = true;
                i += 1;
                loop {
                    match chars.get(i)? {
                        '\'' => break,
                        other => word.push(*other),
                    }
                    i += 1;
                }
            }
            '"' => {
                started = true;
                i += 1;
                loop {
                    match chars.get(i)? {
                        '"' => break,
                        '\\' => match chars.get(i + 1) {
                            Some(next) if matches!(next, '"' | '\\' | '$' | '`') => {
                                word.push(*next);
                                i += 1;
                            }
                            _ => word.push('\\'),
                        },
                        other => {
                            expands |= matches!(other, '$' | '`');
                            word.push(*other);
                        }
                    }
                    i += 1;
                }
            }
            '\\' => match chars.get(i + 1) {
                // A backslash before a line break joins the lines.
                Some('\n') => i += 1,
                Some(next) => {
                    started = true;
                    word.push(*next);
                    i += 1;
                }
                None => {}
            },
            '$' | '`' | '*' | '?' | '[' | '{' => {
                started = true;
                expands = true;
                word.push(c);
            }
            '~' if !started => {
                started = true;
                expands = true;
                word.push(c);
            }
            other => {
                started = true;
                word.push(other);
            }
        }
        i += 1;
    }
    flush(&mut tokens, &mut word, &mut started, &mut expands);
    Some(tokens)
}

fn flush(tokens: &mut Vec<Token>, word: &mut String, started: &mut bool, expands: &mut bool) {
    if *started {
        tokens.push(Token::Word { text: std::mem::take(word), expands: *expands });
    }
    (*started, *expands) = (false, false);
}
