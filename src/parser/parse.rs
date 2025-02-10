use nom::branch::{alt, permutation};
use nom::bytes::complete::tag;
use nom::combinator::{map, opt};
use nom::multi::{many0, many1};
use nom::IResult;

/// 任意の式を表す
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Expression(Box<Expr>),
    CompoundStatement(CompoundStatement),
    Command(Box<Command>),
    Identifier(Identifier),
}
impl Expr {
    /// 式を評価する
    pub fn eval(&self) -> i32 {
        match self {
            _ => 0,
        }
    }
}
#[derive(Debug, PartialEq, Clone)]
struct Expression {
    // 式の集合
    expr: Expr,
}
impl Expression {
    /// 生成する
    pub fn new(val: Expr) -> Expression {
        Expression { expr: val }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct CompoundStatement {
    stmt: Vec<Expr>,
}
impl CompoundStatement {
    /// 生成する
    pub fn new(val: Vec<Expr>) -> CompoundStatement {
        CompoundStatement { stmt: val }
    }

    /// 生成する
    pub fn from(val: Expr) -> CompoundStatement {
        CompoundStatement {
            stmt: Vec::from([val]),
        }
    }
}
// コマンドを表す
#[derive(Debug, PartialEq, Clone)]
pub struct Command(Expr, Expr);
impl Command {
    pub fn new(val: Expr, val2: Expr) -> Command {
        Command(val, val2)
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
    fn parse_constant(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = nom::character::complete::alphanumeric1::<
            &str,
            nom::error::VerboseError<&str>,
        >(input)
        .or_else(|_: nom::Err<nom::error::VerboseError<&str>>| {
            nom::character::complete::multispace1::<&str, nom::error::VerboseError<&str>>(input)
        })
        .map_err(|err| nom::Err::Error((input, nom::error::ErrorKind::MapRes)))?;
        Ok((
            no_used,
            Expr::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_identifier(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = nom::bytes::complete::take_until(";")(input)?;

        Ok((
            no_used,
            Expr::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_command(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = map(
            permutation((Self::parse_constant, tag(" "), Self::parse_identifier)),
            |(command, _, sub_command)| Expr::Command(Box::new(Command::new(command, sub_command))),
        )(input)?;

        Ok((no_used, parsed))
    }

    fn parse_statement(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = Self::parse_command(input)?;
        Ok((no_used, parsed))
    }

    pub fn parse_expr(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = alt((
            Self::parse_statement,
            Self::parse_identifier,
            Self::parse_command,
        ))(input)?;
        Ok((no_used, parsed))
    }
}
