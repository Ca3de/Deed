//! DQL Parser
//!
//! Converts token stream from lexer into AST.

use crate::dql_ast::*;
use crate::dql_lexer::{Lexer, Token};
use crate::transaction::IsolationLevel;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            position: 0,
        }
    }

    /// Parse a DQL query string
    pub fn parse(query: &str) -> Result<Query, String> {
        let mut lexer = Lexer::new(query);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse_query()
    }

    /// Parse top-level query
    pub fn parse_query(&mut self) -> Result<Query, String> {
        match self.current() {
            Token::From => Ok(Query::Select(self.parse_select()?)),
            Token::Insert => Ok(Query::Insert(self.parse_insert()?)),
            Token::Update => Ok(Query::Update(self.parse_update()?)),
            Token::Delete => Ok(Query::Delete(self.parse_delete()?)),
            Token::Create => {
                // Check if this is CREATE INDEX or CREATE edge
                if self.peek() == Some(&Token::Index) || self.peek() == Some(&Token::Unique) {
                    Ok(Query::CreateIndex(self.parse_create_index()?))
                } else {
                    Ok(Query::Create(self.parse_create()?))
                }
            }
            Token::Drop => Ok(Query::DropIndex(self.parse_drop_index()?)),
            Token::Begin => Ok(Query::Begin(self.parse_begin()?)),
            Token::Commit => {
                self.advance();
                Ok(Query::Commit)
            }
            Token::Rollback => {
                self.advance();
                Ok(Query::Rollback)
            }
            _ => Err(format!("Expected query keyword, got {:?}", self.current())),
        }
    }

    /// Parse SELECT query
    fn parse_select(&mut self) -> Result<SelectQuery, String> {
        let from = self.parse_from()?;

        let traverse = if self.current() == &Token::Traverse {
            Some(self.parse_traverse()?)
        } else {
            None
        };

        let where_clause = if self.current() == &Token::Where {
            Some(self.parse_where()?)
        } else {
            None
        };

        self.expect(&Token::Select)?;
        let select = self.parse_select_clause()?;

        let group_by = if self.current() == &Token::GroupBy {
            Some(self.parse_group_by()?)
        } else {
            None
        };

        let having = if self.current() == &Token::Having {
            Some(self.parse_having()?)
        } else {
            None
        };

        let order_by = if self.current() == &Token::OrderBy {
            Some(self.parse_order_by()?)
        } else {
            None
        };

        let limit = if self.current() == &Token::Limit {
            self.advance();
            let limit_value = self.parse_integer()?;
            Some(limit_value as usize)
        } else {
            None
        };

        let offset = if self.current() == &Token::Offset {
            self.advance();
            let offset_value = self.parse_integer()?;
            Some(offset_value as usize)
        } else {
            None
        };

        Ok(SelectQuery {
            from,
            traverse,
            where_clause,
            select,
            group_by,
            having,
            order_by,
            limit,
            offset,
        })
    }

    /// Parse FROM clause
    fn parse_from(&mut self) -> Result<FromClause, String> {
        self.expect(&Token::From)?;

        let collection = self.parse_identifier()?;

        let alias = if let Token::As = self.current() {
            self.advance();
            Some(self.parse_identifier()?)
        } else if let Token::Identifier(_) = self.current() {
            // Implicit alias without AS keyword
            Some(self.parse_identifier()?)
        } else {
            None
        };

        Ok(FromClause { collection, alias })
    }

    /// Parse TRAVERSE clause
    fn parse_traverse(&mut self) -> Result<TraverseClause, String> {
        self.expect(&Token::Traverse)?;

        let mut patterns = Vec::new();

        // Parse traverse patterns until we hit WHERE or SELECT
        while !matches!(self.current(), Token::Where | Token::Select | Token::Eof) {
            patterns.push(self.parse_traverse_pattern()?);

            // Allow comma-separated patterns
            if self.current() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        if patterns.is_empty() {
            return Err("TRAVERSE requires at least one pattern".to_string());
        }

        Ok(TraverseClause { patterns })
    }

    /// Parse single traverse pattern: -[:TYPE]-> alias
    fn parse_traverse_pattern(&mut self) -> Result<TraversePattern, String> {
        // Parse direction
        let direction = match (self.current(), self.peek()) {
            (Token::Minus, Some(Token::LeftBracket)) | (Token::Minus, Some(Token::Arrow)) => {
                self.advance(); // consume '-'
                Direction::Outgoing
            }
            (Token::LeftArrow, _) => {
                self.advance(); // consume '<-'
                Direction::Incoming
            }
            (Token::BiArrow, _) => {
                self.advance(); // consume '<->'
                Direction::Both
            }
            _ => return Err(format!("Expected edge direction, got {:?}", self.current())),
        };

        // Parse edge type: [:TYPE] or [:TYPE*min..max]
        let (edge_type, min_hops, max_hops) = if self.current() == &Token::LeftBracket {
            self.advance(); // consume '['

            // Optional colon before type
            if self.current() == &Token::Colon {
                self.advance();
            }

            let edge_type = if let Token::Identifier(name) = self.current() {
                let t = Some(name.clone());
                self.advance();
                t
            } else {
                None // No type specified
            };

            // Check for variable length: *min..max
            let (min, max) = if self.current() == &Token::Star {
                self.advance();
                // Parse range
                let min = if let Token::Integer(n) = self.current() {
                    let m = *n as usize;
                    self.advance();
                    m
                } else {
                    1
                };

                let max = if self.current() == &Token::Dot {
                    self.advance();
                    if self.current() == &Token::Dot {
                        self.advance();
                    }
                    if let Token::Integer(n) = self.current() {
                        let m = *n as usize;
                        self.advance();
                        m
                    } else {
                        usize::MAX // Unbounded
                    }
                } else {
                    min
                };

                (min, max)
            } else {
                (1, 1) // Default: exactly 1 hop
            };

            self.expect(&Token::RightBracket)?;

            (edge_type, min, max)
        } else {
            (None, 1, 1)
        };

        // Parse arrow for outgoing (already consumed for incoming/both)
        if direction == Direction::Outgoing && self.current() == &Token::Arrow {
            self.advance();
        }

        // Parse target alias
        let target_alias = if let Token::Identifier(name) = self.current() {
            let alias = Some(name.clone());
            self.advance();
            alias
        } else {
            None
        };

        Ok(TraversePattern {
            direction,
            edge_type,
            target_alias,
            min_hops,
            max_hops,
        })
    }

    /// Parse WHERE clause
    fn parse_where(&mut self) -> Result<WhereClause, String> {
        self.expect(&Token::Where)?;
        let condition = self.parse_expression()?;
        Ok(WhereClause { condition })
    }

    /// Parse expression with precedence
    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_and()?;

        while self.current() == &Token::Or {
            self.advance();
            let right = self.parse_and()?;
            left = Expression::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_comparison()?;

        while self.current() == &Token::And {
            self.advance();
            let right = self.parse_comparison()?;
            left = Expression::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_additive()?;

        loop {
            let expr = match self.current() {
                Token::Equal => {
                    self.advance();
                    Expression::Equal(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                Token::NotEqual => {
                    self.advance();
                    Expression::NotEqual(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                Token::LessThan => {
                    self.advance();
                    Expression::LessThan(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                Token::LessThanEq => {
                    self.advance();
                    Expression::LessThanEq(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                Token::GreaterThan => {
                    self.advance();
                    Expression::GreaterThan(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                Token::GreaterThanEq => {
                    self.advance();
                    Expression::GreaterThanEq(Box::new(left.clone()), Box::new(self.parse_additive()?))
                }
                _ => break,
            };
            left = expr;
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let expr = match self.current() {
                Token::Plus => {
                    self.advance();
                    Expression::Add(Box::new(left.clone()), Box::new(self.parse_multiplicative()?))
                }
                Token::Minus => {
                    self.advance();
                    Expression::Subtract(Box::new(left.clone()), Box::new(self.parse_multiplicative()?))
                }
                _ => break,
            };
            left = expr;
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_unary()?;

        loop {
            let expr = match self.current() {
                Token::Star => {
                    self.advance();
                    Expression::Multiply(Box::new(left.clone()), Box::new(self.parse_unary()?))
                }
                Token::Slash => {
                    self.advance();
                    Expression::Divide(Box::new(left.clone()), Box::new(self.parse_unary()?))
                }
                _ => break,
            };
            left = expr;
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression, String> {
        if self.current() == &Token::Not {
            self.advance();
            Ok(Expression::Not(Box::new(self.parse_unary()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        match self.current().clone() {
            // Aggregate functions
            Token::Count => self.parse_aggregate_function(AggregateFunction::Count),
            Token::Sum => self.parse_aggregate_function(AggregateFunction::Sum),
            Token::Avg => self.parse_aggregate_function(AggregateFunction::Avg),
            Token::Min => self.parse_aggregate_function(AggregateFunction::Min),
            Token::Max => self.parse_aggregate_function(AggregateFunction::Max),

            Token::Identifier(name) => {
                self.advance();

                // Check for property reference: entity.property
                if self.current() == &Token::Dot {
                    self.advance();
                    let property = self.parse_identifier()?;
                    Ok(Expression::Property(PropertyRef {
                        entity: Some(name),
                        property,
                    }))
                } else {
                    Ok(Expression::Property(PropertyRef {
                        entity: None,
                        property: name,
                    }))
                }
            }
            Token::Integer(n) => {
                self.advance();
                Ok(Expression::Literal(Literal::Integer(n)))
            }
            Token::Float(f) => {
                self.advance();
                Ok(Expression::Literal(Literal::Float(f)))
            }
            Token::String(s) => {
                self.advance();
                Ok(Expression::Literal(Literal::String(s)))
            }
            Token::True => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(true)))
            }
            Token::False => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(false)))
            }
            Token::Null => {
                self.advance();
                Ok(Expression::Literal(Literal::Null))
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }
            _ => Err(format!("Unexpected token in expression: {:?}", self.current())),
        }
    }

    /// Parse aggregate function call: COUNT(*), SUM(field), etc.
    fn parse_aggregate_function(&mut self, func: AggregateFunction) -> Result<Expression, String> {
        self.advance(); // consume function name
        self.expect(&Token::LeftParen)?;

        let argument = if self.current() == &Token::Star {
            // COUNT(*) - special case
            self.advance();
            Expression::Literal(Literal::Integer(1)) // Placeholder for "count all"
        } else {
            // COUNT(field), SUM(field), etc.
            self.parse_expression()?
        };

        self.expect(&Token::RightParen)?;

        Ok(Expression::Aggregate(func, Box::new(argument)))
    }

    /// Parse SELECT clause
    fn parse_select_clause(&mut self) -> Result<SelectClause, String> {
        let mut fields = Vec::new();

        loop {
            let expression = self.parse_expression()?;

            let alias = if self.current() == &Token::As {
                self.advance();
                Some(self.parse_identifier()?)
            } else {
                None
            };

            fields.push(SelectField { expression, alias });

            if self.current() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        Ok(SelectClause { fields })
    }

    /// Parse ORDER BY clause
    fn parse_order_by(&mut self) -> Result<OrderByClause, String> {
        self.expect(&Token::OrderBy)?;

        let mut fields = Vec::new();

        loop {
            let expression = self.parse_expression()?;

            let ascending = match self.current() {
                Token::Asc => {
                    self.advance();
                    true
                }
                Token::Desc => {
                    self.advance();
                    false
                }
                _ => true, // Default to ascending
            };

            fields.push(OrderByField {
                expression,
                ascending,
            });

            if self.current() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        Ok(OrderByClause { fields })
    }

    /// Parse GROUP BY clause
    fn parse_group_by(&mut self) -> Result<GroupByClause, String> {
        self.expect(&Token::GroupBy)?;

        let mut fields = Vec::new();

        loop {
            let expression = self.parse_expression()?;
            fields.push(expression);

            if self.current() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        Ok(GroupByClause { fields })
    }

    /// Parse HAVING clause
    fn parse_having(&mut self) -> Result<HavingClause, String> {
        self.expect(&Token::Having)?;
        let condition = self.parse_expression()?;
        Ok(HavingClause { condition })
    }

    /// Parse INSERT query
    fn parse_insert(&mut self) -> Result<InsertQuery, String> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;

        let collection = self.parse_identifier()?;

        self.expect(&Token::Values)?;
        self.expect(&Token::LeftParen)?;

        let mut properties = Vec::new();

        // Parse key-value pairs: {key: value, ...}
        if self.current() == &Token::LeftBrace {
            self.advance();

            loop {
                let key = self.parse_identifier()?;
                self.expect(&Token::Colon)?;
                let value = self.parse_literal()?;

                properties.push((key, value));

                if self.current() == &Token::Comma {
                    self.advance();
                } else {
                    break;
                }
            }

            self.expect(&Token::RightBrace)?;
        }

        self.expect(&Token::RightParen)?;

        Ok(InsertQuery {
            collection,
            properties,
        })
    }

    /// Parse UPDATE query
    fn parse_update(&mut self) -> Result<UpdateQuery, String> {
        self.expect(&Token::Update)?;

        let collection = self.parse_identifier()?;

        self.expect(&Token::Set)?;

        let mut set = Vec::new();

        loop {
            let property = self.parse_identifier()?;
            self.expect(&Token::Equal)?;
            let value = self.parse_expression()?;

            set.push((property, value));

            if self.current() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        let where_clause = if self.current() == &Token::Where {
            Some(self.parse_where()?)
        } else {
            None
        };

        Ok(UpdateQuery {
            collection,
            set,
            where_clause,
        })
    }

    /// Parse DELETE query
    fn parse_delete(&mut self) -> Result<DeleteQuery, String> {
        self.expect(&Token::Delete)?;
        self.expect(&Token::From)?;

        let collection = self.parse_identifier()?;

        let where_clause = if self.current() == &Token::Where {
            Some(self.parse_where()?)
        } else {
            None
        };

        Ok(DeleteQuery {
            collection,
            where_clause,
        })
    }

    /// Parse CREATE query
    fn parse_create(&mut self) -> Result<CreateQuery, String> {
        self.expect(&Token::Create)?;

        self.expect(&Token::LeftParen)?;
        let source = self.parse_expression()?;
        self.expect(&Token::RightParen)?;

        self.expect(&Token::Minus)?;
        self.expect(&Token::LeftBracket)?;
        self.expect(&Token::Colon)?;
        let edge_type = self.parse_identifier()?;
        self.expect(&Token::RightBracket)?;
        self.expect(&Token::Arrow)?;

        self.expect(&Token::LeftParen)?;
        let target = self.parse_expression()?;
        self.expect(&Token::RightParen)?;

        let properties = if self.current() == &Token::LeftBrace {
            self.advance();
            let mut props = Vec::new();

            loop {
                let key = self.parse_identifier()?;
                self.expect(&Token::Colon)?;
                let value = self.parse_literal()?;

                props.push((key, value));

                if self.current() == &Token::Comma {
                    self.advance();
                } else {
                    break;
                }
            }

            self.expect(&Token::RightBrace)?;
            props
        } else {
            Vec::new()
        };

        Ok(CreateQuery {
            edge_type,
            source,
            target,
            properties,
        })
    }

    /// Parse BEGIN TRANSACTION query
    fn parse_begin(&mut self) -> Result<BeginQuery, String> {
        self.expect(&Token::Begin)?;

        // Optional TRANSACTION keyword
        if self.current() == &Token::Transaction {
            self.advance();
        }

        // Optional ISOLATION LEVEL clause
        let isolation_level = if self.current() == &Token::Isolation {
            self.advance();
            self.expect(&Token::Level)?;

            let level_name = self.parse_identifier()?;
            Some(match level_name.to_uppercase().as_str() {
                "READ" => {
                    // READ UNCOMMITTED or READ COMMITTED
                    let uncommitted_or_committed = self.parse_identifier()?;
                    match uncommitted_or_committed.to_uppercase().as_str() {
                        "UNCOMMITTED" => IsolationLevel::ReadUncommitted,
                        "COMMITTED" => IsolationLevel::ReadCommitted,
                        _ => return Err(format!("Invalid isolation level: READ {}", uncommitted_or_committed)),
                    }
                }
                "REPEATABLE" => {
                    // REPEATABLE READ
                    let read = self.parse_identifier()?;
                    if read.to_uppercase() != "READ" {
                        return Err(format!("Expected READ after REPEATABLE, got {}", read));
                    }
                    IsolationLevel::RepeatableRead
                }
                "SERIALIZABLE" => IsolationLevel::Serializable,
                _ => return Err(format!("Invalid isolation level: {}", level_name)),
            })
        } else {
            None
        };

        Ok(BeginQuery { isolation_level })
    }

    /// Parse CREATE INDEX or CREATE UNIQUE INDEX
    fn parse_create_index(&mut self) -> Result<CreateIndexQuery, String> {
        self.expect(&Token::Create)?;

        // Check for UNIQUE keyword
        let unique = if self.current() == &Token::Unique {
            self.advance();
            true
        } else {
            false
        };

        self.expect(&Token::Index)?;

        // Index name
        let index_name = self.parse_identifier()?;

        // ON keyword
        self.expect(&Token::On)?;

        // Collection name
        let collection = self.parse_identifier()?;

        // Field name in parentheses
        self.expect(&Token::LeftParen)?;
        let field = self.parse_identifier()?;
        self.expect(&Token::RightParen)?;

        Ok(CreateIndexQuery {
            index_name,
            collection,
            field,
            unique,
        })
    }

    /// Parse DROP INDEX
    fn parse_drop_index(&mut self) -> Result<DropIndexQuery, String> {
        self.expect(&Token::Drop)?;
        self.expect(&Token::Index)?;

        let index_name = self.parse_identifier()?;

        Ok(DropIndexQuery { index_name })
    }

    // Helper methods

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::Eof)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    fn advance(&mut self) {
        if self.position < self.tokens.len() {
            self.position += 1;
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        if std::mem::discriminant(self.current()) == std::mem::discriminant(expected) {
            self.advance();
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", expected, self.current()))
        }
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        if let Token::Identifier(name) = self.current() {
            let result = name.clone();
            self.advance();
            Ok(result)
        } else {
            Err(format!("Expected identifier, got {:?}", self.current()))
        }
    }

    fn parse_integer(&mut self) -> Result<i64, String> {
        if let Token::Integer(n) = self.current() {
            let result = *n;
            self.advance();
            Ok(result)
        } else {
            Err(format!("Expected integer, got {:?}", self.current()))
        }
    }

    fn parse_literal(&mut self) -> Result<Literal, String> {
        match self.current().clone() {
            Token::Integer(n) => {
                self.advance();
                Ok(Literal::Integer(n))
            }
            Token::Float(f) => {
                self.advance();
                Ok(Literal::Float(f))
            }
            Token::String(s) => {
                self.advance();
                Ok(Literal::String(s))
            }
            Token::True => {
                self.advance();
                Ok(Literal::Bool(true))
            }
            Token::False => {
                self.advance();
                Ok(Literal::Bool(false))
            }
            Token::Null => {
                self.advance();
                Ok(Literal::Null)
            }
            _ => Err(format!("Expected literal, got {:?}", self.current())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let query = "FROM Users WHERE age = 25 SELECT name";
        let result = Parser::parse(query).unwrap();

        if let Query::Select(select) = result {
            assert_eq!(select.from.collection, "Users");
            assert!(select.traverse.is_none());
            assert!(select.where_clause.is_some());
        } else {
            panic!("Expected SELECT query");
        }
    }

    #[test]
    fn test_parse_hybrid_query() {
        let query = "FROM Users u TRAVERSE -[:PURCHASED]-> p WHERE p.price > 100 SELECT u.name, p.name";
        let result = Parser::parse(query).unwrap();

        if let Query::Select(select) = result {
            assert_eq!(select.from.collection, "Users");
            assert_eq!(select.from.alias, Some("u".to_string()));
            assert!(select.traverse.is_some());

            let traverse = select.traverse.unwrap();
            assert_eq!(traverse.patterns.len(), 1);
            assert_eq!(
                traverse.patterns[0].edge_type,
                Some("PURCHASED".to_string())
            );
            assert_eq!(traverse.patterns[0].target_alias, Some("p".to_string()));
        } else {
            panic!("Expected SELECT query");
        }
    }

    #[test]
    fn test_parse_variable_length_traverse() {
        let query = "FROM Users TRAVERSE -[:FOLLOWS*1..3]-> friend SELECT friend.name";
        let result = Parser::parse(query).unwrap();

        if let Query::Select(select) = result {
            let traverse = select.traverse.unwrap();
            assert_eq!(traverse.patterns[0].min_hops, 1);
            assert_eq!(traverse.patterns[0].max_hops, 3);
        } else {
            panic!("Expected SELECT query");
        }
    }

    #[test]
    fn test_parse_with_order_and_limit() {
        let query = "FROM Products WHERE price > 50 SELECT name, price ORDER BY price DESC LIMIT 10";
        let result = Parser::parse(query).unwrap();

        if let Query::Select(select) = result {
            assert!(select.order_by.is_some());
            assert_eq!(select.limit, Some(10));

            let order_by = select.order_by.unwrap();
            assert_eq!(order_by.fields[0].ascending, false);
        } else {
            panic!("Expected SELECT query");
        }
    }
}
