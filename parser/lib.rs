pub mod ast;
mod precedences;

pub extern crate lexer;
use lexer::token::{TokenKind, Token};
use lexer::Lexer;
use crate::ast::{Program, Statement, Expression, Node, Literal, BlockStatement};
use crate::precedences::{Precedence, get_token_precedence};

type ParseError = String;
type ParseErrors = Vec<ParseError>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
    errors: ParseErrors,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Parser<'a> {
        let cur = lexer.next_token();
        let next = lexer.next_token();
        let errors = Vec::new();
        // in strict sense, rust can be as classic go pattern, but it requires more work
        // so let's just use pattern matching
        // ```rust
        // type PrefixParseFn = fn() -> Result<Expression, ParseError>;
        // type InfixParseFn = fn(Expression) -> Result<Expression, ParseError>;
        // let prefix_parse_fns = HashMap::new();
        // let infix_parse_fns = HashMap::new();
        // ```

        let p = Parser {
            lexer,
            current_token: cur,
            peek_token: next,
            errors,
        };

        return p;
    }

    fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
    }

    fn current_token_is(&mut self, token: &TokenKind) -> bool {
        self.current_token.kind == *token
    }

    fn peek_token_is(&mut self, token: &TokenKind) -> bool {
        self.peek_token.kind == *token
    }

    fn expect_peek(&mut self, token: &TokenKind) -> Result<(), ParseError> {
        self.next_token();
        if self.current_token.kind == *token {
            Ok(())
        } else {
            let e = format!("expected token: {}, got: {}", token, self.current_token);
            Err(e)
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseErrors> {
        let mut program = Program::new();
        while !self.current_token_is(&TokenKind::EOF) {
            match self.parse_statement() {
                Ok(stmt) => program.statements.push(stmt),
                Err(e) => self.errors.push(e),
            }
            self.next_token();
        }

        if self.errors.is_empty() {
            return Ok(program);
        } else {
            return Err(self.errors.clone());
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.current_token.kind {
            TokenKind::LET => self.parse_let_statement(),
            TokenKind::RETURN => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_let_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();
        let name = match self.current_token.kind {
            TokenKind::IDENTIFIER(ref id) => id.to_string(),
            _ => return Err(format!("not an identifier"))
        };
        self.expect_peek(&TokenKind::ASSIGN)?;
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?;

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Let(name, value));
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?;

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Return(value));
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression(Precedence::LOWEST)?;
        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        Ok(Statement::Expr(expr))
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<Expression, ParseError> {
        let mut left = self.parse_prefix_expression()?;
        while self.peek_token.kind != TokenKind::SEMICOLON && precedence < get_token_precedence(&self.peek_token.kind) {
            match self.parse_infix_expression(&left) {
                Some(infix) => left = infix?,
                None => return Ok(left),
            }
        }

        Ok(left)
    }

    fn parse_prefix_expression(&mut self) -> Result<Expression, ParseError> {
        // this is prefix fn map :)
        match &self.current_token.kind {
            TokenKind::IDENTIFIER(ref id) => return Ok(Expression::IDENTIFIER(id.clone())),
            TokenKind::INT(i) => return Ok(Expression::LITERAL(Literal::Integer(*i))),
            TokenKind::STRING(s) => return Ok(Expression::LITERAL(Literal::String(s.to_string()))),
            b @ TokenKind::TRUE| b @ TokenKind::FALSE => return Ok(Expression::LITERAL(Literal::Boolean(*b == TokenKind::TRUE))),
            TokenKind::BANG | TokenKind::MINUS => {
                let prefix_op = self.current_token.clone();
                self.next_token();
                let expr = self.parse_expression(Precedence::PREFIX)?;
                return Ok(Expression::PREFIX(prefix_op, Box::new(expr)));
            },
            TokenKind::LPAREN => {
                self.next_token();
                let expr = self.parse_expression(Precedence::LOWEST);
                self.expect_peek(&TokenKind::RPAREN)?;
                return expr
            },
            TokenKind::LBRACKET => {
                let elements = self.parse_expression_list(&TokenKind::RBRACKET)?;
                return Ok(Expression::LITERAL(Literal::Array(elements)));
            },
            TokenKind::IF => self.parse_if_expression(),
            TokenKind::FUNCTION => self.parse_fn_expression(),
            _ => {
                Err(format!("no prefix function for token: {}", self.current_token))
            }
        }
    }

    fn parse_infix_expression(&mut self, left: &Expression) -> Option<Result<Expression, ParseError>> {
        match self.peek_token.kind {
            TokenKind::PLUS |
            TokenKind::MINUS |
            TokenKind::ASTERISK |
            TokenKind::SLASH |
            TokenKind::EQ |
            TokenKind::NotEq |
            TokenKind::LT |
            TokenKind::GT => {
                self.next_token();
                let infix_op = self.current_token.clone();
                let precedence_value = get_token_precedence(&self.current_token.kind);
                self.next_token();
                let right: Expression = self.parse_expression(precedence_value).unwrap();
                return Some(Ok(Expression::INFIX(infix_op, Box::new(left.clone()), Box::new(right))));
            },
            TokenKind::LPAREN => {
                self.next_token();
                return Some(self.parse_fn_call_expression(left.clone()));
            },
            _ => None,

        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        self.expect_peek(&TokenKind::LPAREN)?;
        self.next_token();

        let condition = self.parse_expression(Precedence::LOWEST)?;
        self.expect_peek(&TokenKind::RPAREN)?;
        self.expect_peek(&TokenKind::LBRACE)?;

        let consequence = self.parse_block_statement()?;

        let alternative = if self.peek_token_is(&TokenKind::ELSE) {
            self.next_token();
            self.expect_peek(&TokenKind::LBRACE)?;
            Some(self.parse_block_statement()?)
        } else {
            None
        };

        return Ok(Expression::IF(Box::new(condition), consequence, alternative))
    }

    fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        self.next_token();
        let mut block_statement = Vec::new();

        while !self.current_token_is(&TokenKind::RBRACE) && !self.current_token_is(&TokenKind::EOF) {
            if let Ok(statement) = self.parse_statement() {
                block_statement.push(statement)
            }

            self.next_token();
        }

        Ok(BlockStatement::new(block_statement))
    }

    fn parse_fn_expression(&mut self) -> Result<Expression, ParseError> {
        self.expect_peek(&TokenKind::LPAREN)?;

        let params = self.parse_fn_parameters()?;

        self.expect_peek(&TokenKind::LBRACE)?;

        let function_body = self.parse_block_statement()?;

        Ok(Expression::FUNCTION(params, function_body))
    }
    
    fn parse_fn_parameters(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if self.peek_token_is(&TokenKind::RPAREN) {
            self.next_token();
            return Ok(params);
        }

        self.next_token();

        match &self.current_token.kind {
            TokenKind::IDENTIFIER(ref id) => params.push(id.clone()),
            token => return Err(format!("expected function params  to be an identifier, got {}", token))
        }

        while self.peek_token_is(&TokenKind::COMMA) {
           self.next_token();
           self.next_token();
            match &self.current_token.kind {
                TokenKind::IDENTIFIER(ref id) => params.push(id.clone()),
                token => return Err(format!("expected function params  to be an identifier, got {}", token))
            }
        }

        self.expect_peek(&TokenKind::RPAREN)?;

        return Ok(params)
    }

    fn parse_fn_call_expression(&mut self, expr: Expression) -> Result<Expression, ParseError> {
        let arguments = self.parse_expression_list(&TokenKind::RPAREN)?;
        Ok(Expression::FunctionCall(Box::new(expr), arguments))
    }
    
    fn parse_expression_list(&mut self, end: &TokenKind) -> Result<Vec<Expression>, ParseError> {
        let mut expr_list = Vec::new();
        if self.peek_token_is(end) {
            self.next_token();
            return Ok(expr_list);
        }

        self.next_token();

        expr_list.push(self.parse_expression(Precedence::LOWEST)?);

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            expr_list.push(self.parse_expression(Precedence::LOWEST)?);
        }

        self.expect_peek(end)?;

        return Ok(expr_list);
    }
}

pub fn parse(input: &str) -> Result<Node, ParseErrors> {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program()?;

    Ok(Node::Program(program))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_program(test_cases: &[(&str, &str)]) {

        for (input, expected) in test_cases {
            let parsed = parse(input).unwrap().to_string();
            assert_eq!(&format!("{}", parsed), expected);
        }
    }


    #[test]
    fn parse_let_statement() {
        let let_tests = [
            ("let x=5;", "let x = 5;"),
            ("let y=true;", "let y = true;"),
            ("let foo=y;", "let foo = y;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_return_statement() {
        let let_tests = [
            ("return 5", "return 5;"),
            ("return true;", "return true;"),
            ("return foobar;", "return foobar;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_prefix_expression() {
        let let_tests = [
            ("-15;", "(-15)"),
            ("!5;", "(!5)"),
            ("!foobar;", "(!foobar)"),
            ("-foobar;", "(-foobar)"),
            ("!true;", "(!true)"),
            ("!false;", "(!false)"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_infix_expression() {
        let let_tests = [
            ("5 + 5;", "(5 + 5)"),
            ("5 - 5;", "(5 - 5)"),
            ("5 * 5;", "(5 * 5)"),
            ("5 / 5;", "(5 / 5)"),
            ("5 > 5;", "(5 > 5)"),
            ("5 < 5;", "(5 < 5)"),
            ("5 == 5;", "(5 == 5)"),
            ("5 != 5;", "(5 != 5)"),
            ("foobar + barfoo;", "(foobar + barfoo)"),
            ("foobar - barfoo;", "(foobar - barfoo)"),
            ("foobar * barfoo;", "(foobar * barfoo)"),
            ("foobar / barfoo;", "(foobar / barfoo)"),
            ("foobar > barfoo;", "(foobar > barfoo)"),
            ("foobar < barfoo;", "(foobar < barfoo)"),
            ("foobar == barfoo;", "(foobar == barfoo)"),
            ("foobar != barfoo;", "(foobar != barfoo)"),
            ("true == true", "(true == true)"),
            ("true != false", "(true != false)"),
            ("false == false", "(false == false)"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_op_expression() {
        let tt = [
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn parse_brace_expression() {
        let tt = [
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn test_if_expression() {
        let tt = [("if (x < y) { x }", "if (x < y) { x }")];
        verify_program(&tt);
    }

    #[test]
    fn test_if_else_expression() {
        let tt = [("if (x < y) { x } else { y }", "if (x < y) { x } else { y }")];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_else_expression() {
        let tt = [
            ("fn() {};", "fn() {  }"),
            ("fn(x) {};", "fn(x) {  }"),
            ("fn(x, y, z) { x };", "fn(x, y, z) { x }"),
        ];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_call_else_expression() {
        let tt = [
            ("add(1, 2 * 3, 4 + 5);", "add(1, (2 * 3), (4 + 5))")
        ];
        verify_program(&tt);
    }

   #[test]
    fn test_string_literal_expression() {
        let test_case = [(r#""hello world";"#, r#""hello world""#)];
        verify_program(&test_case);
    }

    #[test]
    fn test_array_literal_expression() {
        let test_case = [
            ("[]", "[]"),
            ("[1, 2 * 2, 3 + 3]", "[1, (2 * 2), (3 + 3)]")
        ];
        verify_program(&test_case);
    }

}
