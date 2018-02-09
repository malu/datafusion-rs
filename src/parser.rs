// Copyright 2018 Grove Enterprises LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashSet;
use std::iter::Peekable;
use std::str::Chars;

use super::sql::*;

#[derive(Debug,Clone,PartialEq)]
pub enum Token {
    Identifier(String),
    Keyword(String),
    Operator(String),
    Number(String),
    Comma,
    Whitespace,
    Eq,
    Neq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Plus,
    Minus,
    Mult,
    Div,
    LParen,
    RParen,

    //Operator(String)
}

#[derive(Debug,Clone)]
pub enum ParserError {
    TokenizerError(String),
    ParserError(String),
}

/// SQL keywords
static KEYWORDS : &'static [&'static str] = &[
    "SELECT", "FROM", "WHERE", "LIMIT", "ORDER", "GROUP", "BY", "HAVING",
    "UNION", "ALL", "INSERT", "UPDATE", "DELETE", "IN", "NOT", "NULL",
    "SET", "CREATE", "EXTERNAL", "TABLE", 
    "VARCHAR", "DOUBLE"
];

pub struct Tokenizer {
    keywords: HashSet<String>,
    pub query: String,
}

impl Tokenizer {

    pub fn new(query: &str) -> Self {
        let mut tokenizer = Tokenizer { keywords: HashSet::new(), query: query.to_string() };
        KEYWORDS.into_iter().for_each(|k| {
            tokenizer.keywords.insert(k.to_string());
        });
        tokenizer
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, ParserError> {

        let mut peekable = self.query.chars().peekable();

        let mut tokens : Vec<Token> = vec![];

        while let Some(token) = self.next_token(&mut peekable)? {
            tokens.push(token);
        }

        Ok(tokens.into_iter().filter(|t| match t {
            &Token::Whitespace => false,
            _ => true
        }).collect())
    }

    fn next_token(&self, chars: &mut Peekable<Chars>) -> Result<Option<Token>, ParserError> {
        match chars.peek() {
            Some(&ch) => match ch {
                // whitespace
                ' ' | '\t' | '\n' => {
                    chars.next(); // consume
                    Ok(Some(Token::Whitespace))
                },
                // identifier or keyword
                'a' ... 'z' | 'A' ... 'Z' | '_' | '@' => {
                    let mut s = String::new();
                    while let Some(&ch) = chars.peek() {
                        match ch {
                            'a' ... 'z' | 'A' ... 'Z' | '_' | '0' ... '9' => {
                                chars.next(); // consume
                                s.push(ch);
                            },
                            _ => break
                        }
                    }
                    if self.keywords.contains(&s) {
                        Ok(Some(Token::Keyword(s)))
                    } else {
                        Ok(Some(Token::Identifier(s)))
                    }
                },
                // numbers
                '0' ... '9' => {
                    let mut s = String::new();
                    while let Some(&ch) = chars.peek() {
                        match ch {
                            '0' ... '9' => {
                                chars.next(); // consume
                                s.push(ch);
                            },
                            _ => break
                        }
                    }
                    Ok(Some(Token::Number(s)))
                },
                // punctuation
                ',' => { chars.next(); Ok(Some(Token::Comma)) },
                '(' => { chars.next(); Ok(Some(Token::LParen)) },
                ')' => { chars.next(); Ok(Some(Token::RParen)) },
                // operators
                '+' => { chars.next(); Ok(Some(Token::Plus)) },
                '-' => { chars.next(); Ok(Some(Token::Minus)) },
                '*' => { chars.next(); Ok(Some(Token::Mult)) },
                '/' => { chars.next(); Ok(Some(Token::Div)) },
                '=' => { chars.next(); Ok(Some(Token::Eq)) },
                '<' => {
                    chars.next(); // consume
                    match chars.peek() {
                        Some(&ch) => match ch {
                            '=' => {
                                chars.next();
                                Ok(Some(Token::LtEq))
                            },
                            '>' => {
                                chars.next();
                                Ok(Some(Token::Neq))
                            },
                            _ => Ok(Some(Token::Lt))
                        },
                        None => Ok(Some(Token::Lt))
                    }
                },
                '>' => {
                    chars.next(); // consume
                    match chars.peek() {
                        Some(&ch) => match ch {
                            '=' => {
                                chars.next();
                                Ok(Some(Token::GtEq))
                            },
                            _ => Ok(Some(Token::Gt))
                        },
                        None => Ok(Some(Token::Gt))
                    }
                },
                _ => Err(ParserError::TokenizerError(
                    String::from(format!("unhandled char '{}' in tokenizer", ch))))
            },
            None => Ok(None)
        }
    }
}

pub struct Parser<'a> {
    tokens: &'a Vec<Token>,
    index: usize
}

impl<'a> Parser<'a> {

    pub fn new(tokens: &'a Vec<Token>) -> Self {
        Parser { tokens: tokens, index: 0 }
    }

    pub fn parse_sql(sql: String) -> Result<ASTNode, ParserError> {
        let mut tokenizer = Tokenizer::new(&sql);
        let tokens = tokenizer.tokenize()?;
        let mut parser = Parser::new(&tokens);
        parser.parse()
    }

    pub fn parse(&mut self) -> Result<ASTNode, ParserError> {
        self.parse_expr(0)
    }

    fn parse_expr(&mut self, precedence: u8) -> Result<ASTNode, ParserError> {
        //println!("parse_expr() precendence = {}", precedence);

        let mut expr = self.parse_prefix()?;
        //println!("parsed prefix: {:?}", expr);

        loop {

            let next_precedence = self.get_next_precedence()?;
            if precedence >= next_precedence {
                //println!("break on precedence change ({} >= {})", precedence, next_precedence);
                break;
            }

            if let Some(infix_expr) = self.parse_infix(expr.clone(), next_precedence)? {
                //println!("parsed infix: {:?}", infix_expr);
                expr = infix_expr;
            }
        }

        //println!("parse_expr() returning {:?}", expr);

        Ok(expr)
    }

    fn parse_prefix(&mut self) -> Result<ASTNode, ParserError> {
        match self.next_token() {
            Some(t) => {
                match t {
                    Token::Keyword(k) => {
                        match k.to_uppercase().as_ref() {
                            "SELECT" => Ok(self.parse_select()?),
                            "CREATE" => Ok(self.parse_create()?),
                            _ => Err(ParserError::ParserError(
                                format!("No prefix parser for keyword {}", k))),
                        }
                    },
                    Token::Identifier(id) => {
                        match self.peek_token() {
                            Some(&Token::LParen) => {
                                self.next_token(); // skip lparen

                                let args = self.parse_expr_list()?;

                                self.next_token(); // skip rparen

                                Ok(ASTNode::SQLFunction { id, args })
                            },
                            _ => Ok(ASTNode::SQLIdentifier { id: id })
                        }
                    }
                    Token::Number(n) => match n.parse::<i64>() {
                        Ok(n) => Ok(ASTNode::SQLLiteralInt(n)),
                        Err(e) => Err(ParserError::ParserError(format!(
                            "Could not parse '{}' as i64: {}",
                            n, e
                        ))),
                    },
                    _ => Err(ParserError::ParserError(format!(
                        "Prefix parser expected a keyword but found {:?}",
                        t
                    ))),
                }
            },
            None => Err(ParserError::ParserError(
                format!("Prefix parser expected a keyword but hit EOF")))
        }
    }

    fn parse_infix(&mut self, expr: ASTNode, precedence: u8) -> Result<Option<ASTNode>, ParserError> {
        match self.next_token() {
            Some(tok) => {
                match tok {
                    Token::Eq | Token::Gt | Token::GtEq |
                    Token::Lt | Token::LtEq => Ok(Some(ASTNode::SQLBinaryExpr {
                        left: Box::new(expr),
                        op: self.to_sql_operator(&tok)?,
                        right: Box::new(self.parse_expr(precedence)?)
                    })),
                    _ => Err(ParserError::ParserError(
                        format!("No infix parser for token {:?}", tok))),
                }
            },
            None => Ok(None)
        }
    }

    fn to_sql_operator(&self, tok: &Token) -> Result<SQLOperator, ParserError> {
        match tok {
            &Token::Eq => Ok(SQLOperator::EQ),
            &Token::Lt => Ok(SQLOperator::LT),
            &Token::LtEq => Ok(SQLOperator::LTEQ),
            &Token::Gt => Ok(SQLOperator::GT),
            &Token::GtEq => Ok(SQLOperator::GTEQ),
            //TODO: the rest
            _ => Err(ParserError::ParserError(format!("Unsupported operator {:?}", tok)))
        }
    }

    fn get_next_precedence(&self) -> Result<u8, ParserError> {
        if self.index < self.tokens.len() {
            self.get_precedence(&self.tokens[self.index])
        } else {
            Ok(0)
        }
    }

    fn get_precedence(&self, tok: &Token) -> Result<u8, ParserError> {
        //println!("get_precedence() {:?}", tok);

        match tok {
            &Token::Eq | &Token::Lt | & Token::LtEq |
            &Token::Neq | &Token::Gt | & Token::GtEq => Ok(20),
            &Token::Plus | &Token::Minus => Ok(30),
            &Token::Mult | &Token::Div => Ok(40),
            _ => Ok(0)
                /*Err(ParserError::TokenizerError(
                format!("invalid token {:?} for get_precedence", tok)))*/
        }
    }

    fn peek_token(&mut self) -> Option<&Token> {
        if self.index < self.tokens.len() {
            Some(&self.tokens[self.index])
        } else {
            None
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.index < self.tokens.len() {
            self.index = self.index + 1;
            Some(self.tokens[self.index-1].clone())
        } else {
            None
        }
    }

    fn prev_token(&mut self) -> Option<Token> {
        if self.index > 0 {
            Some(self.tokens[self.index-1].clone())
        } else {
            None
        }
    }

    fn parse_keyword(&mut self, expected: &'static str) -> bool {
        //println!("parse_keyword? {}", expected);
        let b = match self.peek_token() {
            Some(&Token::Keyword(ref k)) => {
                if expected.eq_ignore_ascii_case(k.as_str()) {
                    true
                } else {
                    false
                }
            },
            _ => false
        };

        if b {
            self.index += 1;
        }

        b
    }

    fn parse_keywords(&mut self, keywords: Vec<&'static str>) -> bool {
        let index = self.index;
        for keyword in keywords {
            if !self.parse_keyword(&keyword) {
                self.index = index;
                return false;
            }
        }
        true
    }

//    fn parse_identifier(&mut self) -> Result<ASTNode::SQLIdentifier, Err> {
//        let expr = self.parse_expr()?;
//        match expr {
//            Some(ASTNode::SQLIdentifier { .. }) => Ok(expr),
//            _ => Err(ParserError::ParserError(format!("Expected identifier but found {:?}", expr)))
//        }
//    }

    fn consume_token(&mut self, expected: &Token) -> Result<(), ParserError> {
        match self.next_token() {
            Some(ref t) if *t == *expected => Ok(()),
            _ => Err(ParserError::ParserError(
                    format!("expected token {:?} but was {:?}", expected, self.prev_token())))
        }
    }


    // specific methods

    fn parse_create(&mut self) -> Result<ASTNode, ParserError> {
        if self.parse_keywords(vec!["EXTERNAL", "TABLE"]) {

            match self.next_token() {
                Some(Token::Identifier(id)) => {
                    self.consume_token(&Token::LParen)?;

                    let mut columns = vec![];

                    // parse column defs
                    loop {
                        if let Some(Token::Identifier(column_name)) = self.next_token() {
                            if let Ok(data_type) = self.parse_data_type() {
                                if self.parse_keywords(vec!["NOT", "NULL"]) {
                                    //TODO:
                                } else if self.parse_keyword("NULL") {
                                    //TODO:
                                }

                                match self.peek_token() {
                                    Some(&Token::Comma) => {
                                        self.next_token();
                                        columns.push(SQLColumnDef {
                                            name: column_name,
                                            data_type: data_type,
                                            allow_null: true // TODO
                                        });
                                    },
                                    Some(&Token::RParen) => {
                                        self.next_token();
                                        columns.push(SQLColumnDef {
                                            name: column_name,
                                            data_type: data_type,
                                            allow_null: true // TODO
                                        });
                                        break;
                                    },
                                    _ => return Err(ParserError::ParserError("Expected ',' or ')' after column definition".to_string()))
                                }

                            } else {
                                return Err(ParserError::ParserError("Error parsing data type in column definition".to_string()))
                            }
                        } else {
                            return Err(ParserError::ParserError("Error parsing column name".to_string()))
                        }
                    }

                    Ok(ASTNode::SQLCreateTable {
                        name: id,
                        columns: columns
                    })
                },
                _ => Err(ParserError::ParserError(format!("Unexpected token after CREATE EXTERNAL TABLE: {:?}", self.peek_token())))

            }

        } else {
            Err(ParserError::ParserError(format!("Unexpected token after CREATE: {:?}", self.peek_token())))
        }
    }

    fn parse_literal_int(&mut self) -> Result<i64, ParserError> {
        match self.next_token() {
            Some(Token::Number(s)) => s.parse::<i64>().map_err(|e| {
                ParserError::ParserError(format!("Could not parse '{}' as i64: {}", s, e))
            }),
            _ => Err(ParserError::ParserError(
                "Expected literal int".to_string(),
            )),
        }
    }

    fn parse_data_type(&mut self) -> Result<SQLType, ParserError> {
        match self.next_token() {
            Some(Token::Keyword(k)) => match k.to_uppercase().as_ref() {
                "DOUBLE" => Ok(SQLType::Double),
                "VARCHAR" => {
                    self.consume_token(&Token::LParen)?;
                    let n = self.parse_literal_int()?;
                    self.consume_token(&Token::RParen)?;
                    Ok(SQLType::Varchar(n as usize))
                },
                _ => Err(ParserError::ParserError("Invalid data type".to_string()))
            },
            _ => Err(ParserError::ParserError("Invalid data type".to_string()))
        }
    }

    fn parse_select(&mut self) -> Result<ASTNode, ParserError> {

        let projection = self.parse_expr_list()?;

        let relation : Option<Box<ASTNode>> = if self.parse_keyword("FROM") {
            //TODO: add support for JOIN
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };

        let selection = if self.parse_keyword("WHERE") {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };

        //TODO: parse GROUP BY
        //TODO: parse HAVING
        //TODO: parse ORDER BY

        let limit = if self.parse_keyword("LIMIT") {
            self.parse_limit()?
        } else {
            None
        };

        if let Some(next_token) = self.peek_token() {
            Err(ParserError::ParserError(format!(
                "Unexpected token at end of SELECT: {:?}",
                next_token
            )))
        } else {
            Ok(ASTNode::SQLSelect {
                projection: projection,
                selection: selection,
                relation: relation,
                limit: limit,
                order: None,
            })
        }
    }

    fn helper(&mut self) -> Result<(ASTNode, bool), ParserError> {
        let expr = self.parse_expr(0)?;
        if self.index < self.tokens.len() && self.tokens[self.index] == Token::Comma {
            self.index += 1;
            Ok((expr, true))
        } else {
            Ok((expr, false))
        }
    }

    fn parse_expr_list(&mut self) -> Result<Vec<ASTNode>, ParserError> {
        let mut expr_list : Vec<ASTNode> = vec![];
        loop {
            let (expr, more) = self.helper()?;
            expr_list.push(expr);
            if !more {
                break;
            }
        }

        Ok(expr_list)
    }

    fn parse_limit(&mut self) -> Result<Option<Box<ASTNode>>, ParserError> {
        if self.parse_keyword("ALL") {
            Ok(None)
        } else {
            self.parse_literal_int().map(|n| Some(Box::new(ASTNode::SQLLiteralInt(n))))
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn tokenize(sql: &str) -> Result<Vec<Token>, ParserError> {
        let mut tokenizer = Tokenizer::new(&sql);
        tokenizer.tokenize()
    }

    #[test]
    fn tokenize_select_1()  {
        let tokens = tokenize("SELECT 1");

        let expected = vec![
            Token::Keyword(String::from("SELECT")),
            Token::Number(String::from("1"))
        ];

        assert!(tokens.is_ok());
        compare(expected, tokens.unwrap());
    }

    #[test]
    fn tokenize_scalar_function()  {
        let tokens = tokenize("SELECT sqrt(1)");

        let expected = vec![
            Token::Keyword(String::from("SELECT")),
            Token::Identifier(String::from("sqrt")),
            Token::LParen,
            Token::Number(String::from("1")),
            Token::RParen
        ];

        assert!(tokens.is_ok());
        compare(expected, tokens.unwrap());
    }

    #[test]
    fn tokenize_simple_select() {
        let tokens = tokenize("SELECT * FROM customer WHERE id = 1 LIMIT 5");

        let expected = vec![
            Token::Keyword(String::from("SELECT")),
            Token::Mult,
            Token::Keyword(String::from("FROM")),
            Token::Identifier(String::from("customer")),
            Token::Keyword(String::from("WHERE")),
            Token::Identifier(String::from("id")),
            Token::Eq,
            Token::Number(String::from("1")),
            Token::Keyword(String::from("LIMIT")),
            Token::Number(String::from("5")),
        ];

        assert!(tokens.is_ok());
        compare(expected, tokens.unwrap());
    }

    fn parse_to_ast(sql: &str) -> Result<ASTNode, ParserError> {
        let mut tokenizer = Tokenizer::new(&sql);
        let tokens = tokenizer.tokenize().unwrap();
        let mut parser = Parser::new(&tokens);
        parser.parse()
    }

    #[test]
    fn parse_simple_select() {
        let ast = parse_to_ast("SELECT id, fname, lname FROM customer WHERE id = 1 LIMIT 5");
        if let ASTNode::SQLSelect { projection, limit, ..  } = ast.unwrap() {
            assert_eq!(3, projection.len());
            assert_eq!(Some(Box::new(ASTNode::SQLLiteralInt(5))), limit);
        }
    }

    #[test]
    fn parse_limit_accepts_all() {
        let ast = parse_to_ast("SELECT id, fname, lname FROM customer WHERE id = 1 LIMIT ALL");
        assert!(ast.is_ok());
        if let ASTNode::SQLSelect { projection, limit, .. } = ast.unwrap() {
            assert_eq!(3, projection.len());
            assert_eq!(None, limit);
        }
    }

    #[test]
    fn parse_create_external_table() {
        let ast = parse_to_ast("CREATE EXTERNAL TABLE uk_cities (\
            name VARCHAR(100) NOT NULL,\
            lat DOUBLE NOT NULL,\
            lng DOUBLE NOT NULL)");

        assert!(ast.is_ok());
        if let ASTNode::SQLCreateTable { name, columns, .. } = ast.unwrap() {
            assert_eq!("uk_cities", name);
            assert_eq!(3, columns.len());
        }
    }

    #[test]
    fn parse_scalar_function_in_projection() {
        let ast = parse_to_ast("SELECT sqrt(id) FROM foo");
        assert!(ast.is_ok());
        if let ASTNode::SQLSelect { projection, .. } = ast.unwrap() {
            assert_eq!(vec![
                       ASTNode::SQLFunction {
                           id: String::from("sqrt"),
                           args: vec![ASTNode::SQLIdentifier { id: String::from("id") }],
                       }],
                       projection);
        }
    }

    fn compare(expected: Vec<Token>, actual: Vec<Token>) {
        //println!("------------------------------");
        //println!("tokens   = {:?}", actual);
        //println!("expected = {:?}", expected);
        //println!("------------------------------");
        assert_eq!(expected, actual);
    }

}

