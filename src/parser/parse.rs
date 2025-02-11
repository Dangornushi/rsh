use nom::branch::{alt, permutation};
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::{alpha0, multispace0};
use nom::character::streaming::multispace1;
use nom::combinator::{map, opt};
use nom::multi::{many0, many1};
use nom::sequence::delimited;
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
    pub fn eval(&self) -> Vec<Expr> {
        match self {
            Expr::CompoundStatement(compound_statement) => compound_statement.eval(),
            _ => vec![],
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

    /// 生成する
    pub fn start_node(val: Expr, val2: Vec<Expr>) -> CompoundStatement {
        let mut v = val2.clone();
        v.insert(0, val.clone());
        CompoundStatement { stmt: v }
    }
    pub fn eval(&self) -> Vec<Expr> {
        self.stmt.clone()
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
        //let (no_used, parsed) = nom::bytes::complete::take_until(";")(input)?;
        let (no_used, parsed) = nom::bytes::complete::is_not(";")(input)?;

        Ok((
            no_used,
            Expr::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_command(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = map(
            permutation((
                Self::parse_constant,
                opt(permutation((multispace0, Self::parse_identifier))),
            )),
            |(command, opttion)| {
                if let Some((_, sub_command)) = opttion {
                    Expr::Command(Box::new(Command::new(command, sub_command)))
                } else {
                    Expr::Command(Box::new(Command::new(
                        command,
                        Expr::Identifier(Identifier::new("".to_string())),
                    )))
                }
            },
        )(input)?;

        Ok((no_used, parsed))
    }

    fn parse_statement(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = Self::parse_command(input)?;
        Ok((no_used, parsed))
    }

    fn parse_compound_statement(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = map(
            many1(permutation((
                multispace0,
                Self::parse_statement,
                opt(tag(";")),
                multispace0,
            ))),
            |compound_statements| {
                let mut cmpnd_stmts = Vec::new();
                for statement in compound_statements {
                    cmpnd_stmts.push(statement.1);
                }
                Expr::CompoundStatement(CompoundStatement::new(cmpnd_stmts))
            },
        )(input)?;
        Ok((no_used, parsed))
    }

    pub fn parse_expr(input: &str) -> IResult<&str, Expr> {
        let (no_used, parsed) = alt((
            Self::parse_compound_statement,
            Self::parse_compound_statement,
        ))(input)?;
        Ok((no_used, parsed))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        let input = "echo         hello";
        let expected = Expr::Command(Box::new(Command(
            Expr::Identifier(Identifier::new("echo".to_string())),
            Expr::Identifier(Identifier::new("hello".to_string())),
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));
        let input = "echo \"だんごむし\"";
        let expected = Expr::Command(Box::new(Command(
            Expr::Identifier(Identifier::new("echo".to_string())),
            Expr::Identifier(Identifier::new("\"だんごむし\"".to_string())),
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "echo hello";
        let expected = Expr::Command(Box::new(Command(
            Expr::Identifier(Identifier::new("echo".to_string())),
            Expr::Identifier(Identifier::new("hello".to_string())),
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "echo";
        let expected = Expr::Command(Box::new(Command(
            Expr::Identifier(Identifier::new("echo".to_string())),
            Expr::Identifier(Identifier::new("".to_string())),
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));
    }

    #[test]
    fn parse_compound_statement() {
        let input = "echo \"aaaa\"; echo \"だんごむし\"";
        let expected = Expr::CompoundStatement(CompoundStatement::new(vec![
            Expr::Command(Box::new(Command(
                Expr::Identifier(Identifier::new("echo".to_string())),
                Expr::Identifier(Identifier::new("\"aaaa\"".to_string())),
            ))),
            Expr::Command(Box::new(Command(
                Expr::Identifier(Identifier::new("echo".to_string())),
                Expr::Identifier(Identifier::new("\"だんごむし\"".to_string())),
            ))),
        ]));

        let result = Parse::parse_compound_statement(input);
        assert_eq!(result, Ok(("", expected)));
        let input = "echo hello; echo world";
        let expected = Expr::CompoundStatement(CompoundStatement::new(vec![
            Expr::Command(Box::new(Command(
                Expr::Identifier(Identifier::new("echo".to_string())),
                Expr::Identifier(Identifier::new("hello".to_string())),
            ))),
            Expr::Command(Box::new(Command(
                Expr::Identifier(Identifier::new("echo".to_string())),
                Expr::Identifier(Identifier::new("world".to_string())),
            ))),
        ]));
        let result = Parse::parse_compound_statement(input);
        assert_eq!(result, Ok(("", expected)));
    }
}
