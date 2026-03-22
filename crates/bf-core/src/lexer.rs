#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub line: usize,   // 1-based
    pub column: usize, // 1-based
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Plus,
    Minus,
    Left,
    Right,
    LoopStart,
    LoopEnd,
    Input,
    Output,
    Comment { text: String, kind: CommentKind },
    Invalid(char),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut line = 1usize;
    let mut column = 1usize;

    while i < chars.len() {
        let ch = chars[i];
        let pos = Position { line, column };

        match ch {
            '/' => {
                if i + 1 < chars.len() && chars[i + 1] == '/' {
                    // Line comment
                    let mut text = String::new();
                    i += 2;
                    column += 2;
                    while i < chars.len() && chars[i] != '\n' {
                        text.push(chars[i]);
                        i += 1;
                        column += 1;
                    }
                    tokens.push(Token {
                        kind: TokenKind::Comment { text, kind: CommentKind::Line },
                        position: pos,
                    });
                } else if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // Block comment
                    let mut text = String::new();
                    i += 2;
                    column += 2;
                    loop {
                        if i >= chars.len() {
                            break;
                        }
                        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                            i += 2;
                            column += 2;
                            break;
                        }
                        if chars[i] == '\n' {
                            text.push('\n');
                            i += 1;
                            line += 1;
                            column = 1;
                        } else if chars[i] == '\r' && i + 1 < chars.len() && chars[i + 1] == '\n' {
                            text.push('\n');
                            i += 2;
                            line += 1;
                            column = 1;
                        } else {
                            text.push(chars[i]);
                            i += 1;
                            column += 1;
                        }
                    }
                    tokens.push(Token {
                        kind: TokenKind::Comment { text, kind: CommentKind::Block },
                        position: pos,
                    });
                } else {
                    tokens.push(Token { kind: TokenKind::Invalid(ch), position: pos });
                    i += 1;
                    column += 1;
                }
            }
            '+' => {
                tokens.push(Token { kind: TokenKind::Plus, position: pos });
                i += 1;
                column += 1;
            }
            '-' => {
                tokens.push(Token { kind: TokenKind::Minus, position: pos });
                i += 1;
                column += 1;
            }
            '<' => {
                tokens.push(Token { kind: TokenKind::Left, position: pos });
                i += 1;
                column += 1;
            }
            '>' => {
                tokens.push(Token { kind: TokenKind::Right, position: pos });
                i += 1;
                column += 1;
            }
            '[' => {
                tokens.push(Token { kind: TokenKind::LoopStart, position: pos });
                i += 1;
                column += 1;
            }
            ']' => {
                tokens.push(Token { kind: TokenKind::LoopEnd, position: pos });
                i += 1;
                column += 1;
            }
            ',' => {
                tokens.push(Token { kind: TokenKind::Input, position: pos });
                i += 1;
                column += 1;
            }
            '.' => {
                tokens.push(Token { kind: TokenKind::Output, position: pos });
                i += 1;
                column += 1;
            }
            '\n' => {
                line += 1;
                column = 1;
                i += 1;
            }
            '\r' => {
                // Handle \r\n
                if i + 1 < chars.len() && chars[i + 1] == '\n' {
                    i += 2;
                } else {
                    i += 1;
                }
                line += 1;
                column = 1;
            }
            ' ' | '\t' => {
                i += 1;
                column += 1;
            }
            _ => {
                tokens.push(Token { kind: TokenKind::Invalid(ch), position: pos });
                i += 1;
                column += 1;
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn basic_bf() {
        let tokens = tokenize(">+<-");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Right);
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Left);
        assert_eq!(tokens[3].kind, TokenKind::Minus);
    }

    #[test]
    fn line_comment() {
        let tokens = tokenize("+ // increment\n-");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Plus);
        assert!(matches!(&tokens[1].kind, TokenKind::Comment { kind: CommentKind::Line, .. }));
        assert_eq!(tokens[2].kind, TokenKind::Minus);
    }

    #[test]
    fn block_comment() {
        let tokens = tokenize("+ /* hello */ -");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[1].kind, TokenKind::Comment { kind: CommentKind::Block, .. }));
    }

    #[test]
    fn invalid_chars() {
        let tokens = tokenize("@#$");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0].kind, TokenKind::Invalid('@')));
        assert!(matches!(tokens[1].kind, TokenKind::Invalid('#')));
        assert!(matches!(tokens[2].kind, TokenKind::Invalid('$')));
    }

    #[test]
    fn long_minus_run() {
        let tokens = tokenize("---");
        assert_eq!(tokens.len(), 3);
        for t in &tokens {
            assert_eq!(t.kind, TokenKind::Minus);
        }
    }

    #[test]
    fn multiline_positions() {
        let tokens = tokenize("+\n-");
        assert_eq!(tokens[0].position, Position { line: 1, column: 1 });
        assert_eq!(tokens[1].position, Position { line: 2, column: 1 });
    }

    #[test]
    fn crlf_line_endings() {
        let tokens = tokenize("+\r\n-");
        assert_eq!(tokens[0].position, Position { line: 1, column: 1 });
        assert_eq!(tokens[1].position, Position { line: 2, column: 1 });
    }

    #[test]
    fn all_bf_ops() {
        let tokens = tokenize("+-<>[].,");
        let expected = vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Left,
            TokenKind::Right,
            TokenKind::LoopStart,
            TokenKind::LoopEnd,
            TokenKind::Output, // '.' comes before ','
            TokenKind::Input,
        ];
        assert_eq!(tokens.len(), expected.len());
        for (t, e) in tokens.iter().zip(expected.iter()) {
            assert_eq!(&t.kind, e);
        }
    }
}
