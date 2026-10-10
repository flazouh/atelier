use crate::enums::PathCmd;

/// Reads the `d` attribute of a path. It knows M L H V Q Z, in both cases, which is all the data uses.
pub(crate) fn parse_path_data(d: &str) -> Result<Vec<PathCmd>, String> {
    let mut out = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut cmd = ' ';
    let mut args: Vec<f32> = Vec::new();
    let mut token = String::new();
    let mut tokens: Vec<(char, Vec<f32>)> = Vec::new();
    let flush_number = |token: &mut String, args: &mut Vec<f32>| -> Result<(), String> {
        if !token.is_empty() {
            args.push(
                token
                    .parse::<f32>()
                    .map_err(|_| format!("bad number `{token}` in path"))?,
            );
            token.clear();
        }
        Ok(())
    };
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            flush_number(&mut token, &mut args)?;
            if cmd != ' ' {
                tokens.push((cmd, std::mem::take(&mut args)));
            }
            cmd = ch;
        } else if ch == '-' {
            flush_number(&mut token, &mut args)?;
            token.push(ch);
        } else if ch == ',' || ch.is_whitespace() {
            flush_number(&mut token, &mut args)?;
        } else {
            token.push(ch);
        }
    }
    flush_number(&mut token, &mut args)?;
    if cmd != ' ' {
        tokens.push((cmd, args));
    }
    for (cmd, args) in tokens {
        let rel = cmd.is_ascii_lowercase();
        let need = |n: usize| {
            if args.len() % n == 0 && !args.is_empty() {
                Ok(())
            } else {
                Err(format!("path `{cmd}` needs groups of {n} numbers"))
            }
        };
        // A relative step adds to the point the path is at when the step starts, so each pair builds on the last.
        let base = |x: f32, y: f32| if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                need(2)?;
                for (i, p) in args.chunks(2).enumerate() {
                    let (bx, by) = base(x, y);
                    (x, y) = (bx + p[0], by + p[1]);
                    if i == 0 {
                        out.push(PathCmd::Move(x, y));
                        (sx, sy) = (x, y);
                    } else {
                        out.push(PathCmd::Line(x, y));
                    }
                }
            }
            'L' => {
                need(2)?;
                for p in args.chunks(2) {
                    let (bx, by) = base(x, y);
                    (x, y) = (bx + p[0], by + p[1]);
                    out.push(PathCmd::Line(x, y));
                }
            }
            'H' => {
                need(1)?;
                for p in &args {
                    x = base(x, y).0 + p;
                    out.push(PathCmd::Line(x, y));
                }
            }
            'V' => {
                need(1)?;
                for p in &args {
                    y = base(x, y).1 + p;
                    out.push(PathCmd::Line(x, y));
                }
            }
            'Q' => {
                need(4)?;
                for p in args.chunks(4) {
                    let (bx, by) = base(x, y);
                    out.push(PathCmd::Quad(bx + p[0], by + p[1], bx + p[2], by + p[3]));
                    (x, y) = (bx + p[2], by + p[3]);
                }
            }
            'Z' => {
                out.push(PathCmd::Close);
                (x, y) = (sx, sy);
            }
            other => return Err(format!("path command `{other}` is not supported")),
        }
    }
    Ok(out)
}
