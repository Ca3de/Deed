//! DQL (Deed Query Language) Lexer
//!
//! Tokenizes DQL queries for parsing. DQL is a unified query language
//! that combines relational (FROM/WHERE/SELECT) and graph (TRAVERSE) operations.
//!
//! Example DQL query:
//! ```dql
//! FROM Users WHERE city = 'NYC'
//! TRAVERSE -[:PURCHASED]-> Product
//! WHERE Product.price > 100
//! SELECT User.name, Product.name, Product.price;
//! ```

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    From,
    Where,
    Select,
    Traverse,
    Create,
    Update,
    Delete,
    Set,
    Insert,
    Into,
    Values,
    And,
    Or,
    Not,
    As,
    Limit,
    Offset,
    OrderBy,
    Asc,
    Desc,

    // Literals
    Identifier(String),
    String(String),
    Integer(i64),
    Float(f64),
    True,
    False,
    Null,

    // Operators
    Equal,           // =
    NotEqual,        // !=
    LessThan,        // <
    LessThanEq,      // <=
    GreaterThan,     // >
    GreaterThanEq,   // >=
    Plus,            // +
    Minus,           // -
    Star,            // *
    Slash,           // /
    Dot,             // .
    Comma,           // ,
    Semicolon,       // ;
    Colon,           // :

    // Graph operators
    Arrow,           // ->
    LeftArrow,       // <-
    BiArrow,         // <->

    // Brackets
    LeftParen,       // (
    RightParen,      // )
    LeftBracket,     // [
    RightBracket,    // ]
    LeftBrace,       // {
    RightBrace,      // }

    // Special
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Identifier(s) => write!(f, "Identifier({})", s),
            Token::String(s) => write!(f, "String(\"{}\")", s),
            Token::Integer(n) => write!(f, "Integer({})", n),
            Token::Float(n) => write!(f, "Float({})", n),
            _ => write!(f, "{:?}", self),
        }
    }
}

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current = chars.get(0).copied();
        Lexer {
            input: chars,
            position: 0,
            current_char: current,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }

        Ok(tokens)
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token, String> {
        // Skip whitespace
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }

        match self.current_char {
            None => Ok(Token::Eof),
            Some(ch) => {
                if ch.is_alphabetic() || ch == '_' {
                    self.read_identifier()
                } else if ch.is_numeric() {
                    self.read_number()
                } else if ch == '\'' || ch == '"' {
                    self.read_string()
                } else {
                    self.read_operator()
                }
            }
        }
    }

    fn advance(&mut self) {
        self.position += 1;
        self.current_char = self.input.get(self.position).copied();
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn read_identifier(&mut self) -> Result<Token, String> {
        let mut result = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_alphanumeric() || ch == '_' {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for keywords (case-insensitive)
        let upper = result.to_uppercase();
        let token = match upper.as_str() {
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "SELECT" => Token::Select,
            "TRAVERSE" => Token::Traverse,
            "CREATE" => Token::Create,
            "UPDATE" => Token::Update,
            "DELETE" => Token::Delete,
            "SET" => Token::Set,
            "INSERT" => Token::Insert,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "AS" => Token::As,
            "LIMIT" => Token::Limit,
            "OFFSET" => Token::Offset,
            "ORDER" => {
                // Check for "ORDER BY"
                self.skip_whitespace();
                if let Some(next_token) = self.peek_identifier() {
                    if next_token.to_uppercase() == "BY" {
                        self.read_identifier()?; // consume BY
                        return Ok(Token::OrderBy);
                    }
                }
                Token::Identifier(result)
            }
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "TRUE" => Token::True,
            "FALSE" => Token::False,
            "NULL" => Token::Null,
            _ => Token::Identifier(result),
        };

        Ok(token)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek_identifier(&self) -> Option<String> {
        let mut pos = self.position;
        let mut result = String::new();

        while let Some(&ch) = self.input.get(pos) {
            if ch.is_alphanumeric() || ch == '_' {
                result.push(ch);
                pos += 1;
            } else {
                break;
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let mut result = String::new();
        let mut is_float = false;

        while let Some(ch) = self.current_char {
            if ch.is_numeric() {
                result.push(ch);
                self.advance();
            } else if ch == '.' && !is_float {
                // Check if next char is a digit (not a method call)
                if let Some(next) = self.peek() {
                    if next.is_numeric() {
                        is_float = true;
                        result.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if is_float {
            result
                .parse::<f64>()
                .map(Token::Float)
                .map_err(|e| format!("Invalid float: {}", e))
        } else {
            result
                .parse::<i64>()
                .map(Token::Integer)
                .map_err(|e| format!("Invalid integer: {}", e))
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        let quote_char = self.current_char.unwrap();
        self.advance(); // skip opening quote

        let mut result = String::new();
        let mut escaped = false;

        while let Some(ch) = self.current_char {
            if escaped {
                match ch {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '\'' => result.push('\''),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(ch);
                    }
                }
                escaped = false;
                self.advance();
            } else if ch == '\\' {
                escaped = true;
                self.advance();
            } else if ch == quote_char {
                self.advance(); // skip closing quote
                return Ok(Token::String(result));
            } else {
                result.push(ch);
                self.advance();
            }
        }

        Err("Unterminated string literal".to_string())
    }

    fn read_operator(&mut self) -> Result<Token, String> {
        let ch = self.current_char.unwrap();
        let next = self.peek();

        let token = match (ch, next) {
            ('=', _) => {
                self.advance();
                Token::Equal
            }
            ('!', Some('=')) => {
                self.advance();
                self.advance();
                Token::NotEqual
            }
            ('<', Some('=')) => {
                self.advance();
                self.advance();
                Token::LessThanEq
            }
            ('<', Some('-')) => {
                self.advance();
                self.advance();
                // Check for <->
                if self.current_char == Some('>') {
                    self.advance();
                    Token::BiArrow
                } else {
                    Token::LeftArrow
                }
            }
            ('<', _) => {
                self.advance();
                Token::LessThan
            }
            ('>', Some('=')) => {
                self.advance();
                self.advance();
                Token::GreaterThanEq
            }
            ('>', _) => {
                self.advance();
                Token::GreaterThan
            }
            ('-', Some('>')) => {
                self.advance();
                self.advance();
                Token::Arrow
            }
            ('-', _) => {
                self.advance();
                Token::Minus
            }
            ('+', _) => {
                self.advance();
                Token::Plus
            }
            ('*', _) => {
                self.advance();
                Token::Star
            }
            ('/', _) => {
                self.advance();
                Token::Slash
            }
            ('.', _) => {
                self.advance();
                Token::Dot
            }
            (',', _) => {
                self.advance();
                Token::Comma
            }
            (';', _) => {
                self.advance();
                Token::Semicolon
            }
            (':', _) => {
                self.advance();
                Token::Colon
            }
            ('(', _) => {
                self.advance();
                Token::LeftParen
            }
            (')', _) => {
                self.advance();
                Token::RightParen
            }
            ('[', _) => {
                self.advance();
                Token::LeftBracket
            }
            (']', _) => {
                self.advance();
                Token::RightBracket
            }
            ('{', _) => {
                self.advance();
                Token::LeftBrace
            }
            ('}', _) => {
                self.advance();
                Token::RightBrace
            }
            _ => return Err(format!("Unexpected character: {}", ch)),
        };

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("FROM WHERE SELECT TRAVERSE");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::From);
        assert_eq!(tokens[1], Token::Where);
        assert_eq!(tokens[2], Token::Select);
        assert_eq!(tokens[3], Token::Traverse);
    }

    #[test]
    fn test_identifiers() {
        let mut lexer = Lexer::new("users_table column_name _private");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Identifier("users_table".to_string()));
        assert_eq!(tokens[1], Token::Identifier("column_name".to_string()));
        assert_eq!(tokens[2], Token::Identifier("_private".to_string()));
    }

    #[test]
    fn test_numbers() {
        let mut lexer = Lexer::new("42 3.14 0 -1");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Integer(42));
        assert_eq!(tokens[1], Token::Float(3.14));
        assert_eq!(tokens[2], Token::Integer(0));
        assert_eq!(tokens[3], Token::Minus);
        assert_eq!(tokens[4], Token::Integer(1));
    }

    #[test]
    fn test_strings() {
        let mut lexer = Lexer::new(r#"'hello' "world" 'it\'s'"#);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::String("hello".to_string()));
        assert_eq!(tokens[1], Token::String("world".to_string()));
        assert_eq!(tokens[2], Token::String("it's".to_string()));
    }

    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("= != < <= > >= -> <- <->");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Equal);
        assert_eq!(tokens[1], Token::NotEqual);
        assert_eq!(tokens[2], Token::LessThan);
        assert_eq!(tokens[3], Token::LessThanEq);
        assert_eq!(tokens[4], Token::GreaterThan);
        assert_eq!(tokens[5], Token::GreaterThanEq);
        assert_eq!(tokens[6], Token::Arrow);
        assert_eq!(tokens[7], Token::LeftArrow);
        assert_eq!(tokens[8], Token::BiArrow);
    }

    #[test]
    fn test_hybrid_query() {
        let query = "FROM Users WHERE city = 'NYC' TRAVERSE -[:PURCHASED]-> Product";
        let mut lexer = Lexer::new(query);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::From);
        assert_eq!(tokens[1], Token::Identifier("Users".to_string()));
        assert_eq!(tokens[2], Token::Where);
        assert_eq!(tokens[3], Token::Identifier("city".to_string()));
        assert_eq!(tokens[4], Token::Equal);
        assert_eq!(tokens[5], Token::String("NYC".to_string()));
        assert_eq!(tokens[6], Token::Traverse);
        assert_eq!(tokens[7], Token::Minus);
        assert_eq!(tokens[8], Token::LeftBracket);
        assert_eq!(tokens[9], Token::Colon);
        assert_eq!(tokens[10], Token::Identifier("PURCHASED".to_string()));
        assert_eq!(tokens[11], Token::RightBracket);
        assert_eq!(tokens[12], Token::Arrow);
        assert_eq!(tokens[13], Token::Identifier("Product".to_string()));
    }
}
