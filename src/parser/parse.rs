use nom::bytes::complete::tag;
use nom::character::complete::alpha1;
use nom::combinator::all_consuming;
use nom::multi::separated_list1;
use nom::{IResult, Parser};

/// 任意の式を表す
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    ExprStatement(Vec<Expr>),
    Identifier(Identifier),
}

impl Expr {
    /// 式を評価する
    pub fn eval(&self) -> i32 {
        match self {
            Expr::ExprStatement(_) => 0,
            _ => 0,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct ExprStatement {
    // 式の集合
    expr_stmt: Vec<Expr>,
}
impl ExprStatement {
    /// 生成する
    pub fn new(val: Vec<Expr>) -> ExprStatement {
        ExprStatement { expr_stmt: val }
    }
}

/// 文字列を表す
#[derive(Debug, PartialEq, Clone)]
pub struct Identifier(String);
impl Identifier {
    /// ConstantVal init
    pub fn new(val: String) -> Identifier {
        Identifier(val)
    }

    /// Identifierの値を取得
    pub fn eval(&self) -> String {
        self.0.clone()
    }
}

pub struct Parse {}

impl Parse {
    fn new() -> Parse {
        Parse {}
    }

    pub fn parse_identifier(input: &str) -> IResult<&str, Identifier> {
        let (no_used, parsed) = alpha1(input)?;

        Ok((no_used, Identifier::new(parsed.to_string())))
    }

    pub fn parse_expr_statement(input: &str) -> IResult<&str, ExprStatement> {
        let (no_used, list) = separated_list1(tag(";"), Self::parse_identifier).parse(input)?;
        Ok((
            no_used,
            ExprStatement::new(list.into_iter().map(Expr::Identifier).collect()),
        ))
    }

    pub fn parse_expr(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = all_consuming(Self::parse_expr_statement).parse(input)?;
        Ok((no_used, Expr::eval(parsed.expr_stmt)))
    }
}
