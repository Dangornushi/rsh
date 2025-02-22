use nom::branch::{alt, permutation};
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0};
use nom::combinator::{map, opt};
use nom::multi::{many0, many1};
use nom::IResult;

/// 任意の式を表す
#[derive(Debug, PartialEq, Clone)]
pub enum Node {
    Expression(Box<Node>),
    CompoundStatement(CompoundStatement),
    Statement(Statement),
    Define(Box<Define>),
    Command(Box<Command>),
    Identifier(Identifier),
}
impl Node {
    /// 式を評価する
    pub fn eval(&self) -> Vec<Node> {
        match self {
            Node::CompoundStatement(compound_statement) => compound_statement.eval(),
            Node::Command(command) => command.0.eval(),
            _ => vec![],
        }
    }
}
#[derive(Debug, PartialEq, Clone)]
struct Expression {
    // 式の集合
    node: Node,
}
impl Expression {
    /// 生成する
    pub fn new(val: Node) -> Expression {
        Expression { node: val }
    }
}

// コマンド達の連結を表す
#[derive(Debug, PartialEq, Clone)]
pub struct CompoundStatement {
    stmt: Vec<Node>,
}
impl CompoundStatement {
    /// 生成する
    pub fn new(val: Vec<Node>) -> CompoundStatement {
        CompoundStatement { stmt: val }
    }

    /// 生成する
    pub fn from(val: Node) -> CompoundStatement {
        CompoundStatement {
            stmt: Vec::from([val]),
        }
    }

    /// 生成する
    pub fn start_node(val: Node, val2: Vec<Node>) -> CompoundStatement {
        let mut v = val2.clone();
        v.insert(0, val.clone());
        CompoundStatement { stmt: v }
    }
    pub fn eval(&self) -> Vec<Node> {
        self.stmt.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Statement(Vec<Node>);
impl Statement {
    pub fn new(val: Vec<Node>) -> Statement {
        Statement(val)
    }
    pub fn from(val: Node) -> Statement {
        Statement(Vec::from([val]))
    }
    pub fn eval(&self) -> Vec<Node> {
        self.0.clone()
    }
}

// 代入を表す
#[derive(Debug, PartialEq, Clone)]
pub struct Define{
    var: Node,
    data: Node,
}
impl Define {
    pub fn new(var: Node, data: Node) -> Self {
        Define { var, data }
    }
    pub fn get_var(&self) -> Node {
        self.var.clone()
    }

    pub fn get_data(&self) -> Node {
        self.data.clone()
    }
}

// コマンドを表す
#[derive(Debug, PartialEq, Clone)]
pub struct Command(Node, Vec<Node>);
impl Command {
    pub fn new(val: Node, val2: Vec<Node>) -> Command {
        Command(val, val2)
    }
    pub fn get_command(&self) -> Node {
        self.0.clone()
    }

    pub fn get_sub_command(&self) -> Vec<Node> {
        self.1.clone()
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
    fn parse_constant(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = nom::bytes::complete::is_not(" ;:!?\\/*+~=[](){}<>@^&.,`#^%|")(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_identifier(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) =
            nom::sequence::delimited(tag("\""), nom::bytes::complete::is_not("\""), tag("\""))(
                input,
            )?;

        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_command(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                Self::parse_constant,
                many0(permutation((
                    multispace0,
                    alt((
                        Self::parse_identifier, // "に囲まれている文字列
                        Self::parse_constant,
                    )),
                ))),
            )),
            |(command, options)| {
                if options.len() > 0 {
                    let mut v: Vec<Node> = Vec::new();
                    for opt in options {
                        v.push(opt.1.clone());
                    }
                    Node::Command(Box::new(Command::new(command, v)))
                } else {
                    Node::Command(Box::new(Command::new(command, Vec::new())))
                }
            },
        )(input)?;

        Ok((no_used, parsed))
    }

    fn parse_define(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                multispace0,
                Self::parse_constant,
                multispace0,
                tag("="),
                multispace0,
                Self::parse_identifier,
                multispace0,
            )),
            |(_, var, _, _, _,data,_)| {
                Node::Define(Box::new(Define::new(var, data)))
            }
        )(input)?;
        Ok((no_used, parsed))
    }

    fn parse_statement(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = alt((Self::parse_define, Self::parse_command))(input)?;
        Ok((no_used, parsed))
    }

    fn parse_compound_statement(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            many1(permutation((
                multispace0,
                Self::parse_statement,
                multispace0,
                opt(tag(";")),
                multispace0,
            ))),
            |compound_statements| {
                let mut cmpnd_stmts = Vec::new();
                for statement in compound_statements {
                    cmpnd_stmts.push(statement.1);
                }
                Node::CompoundStatement(CompoundStatement::new(cmpnd_stmts))
            },
        )(input)?;
        Ok((no_used, parsed))
    }

    pub fn parse_node(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) =Self::parse_compound_statement(input)?;
        Ok((no_used, parsed))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identifier() {
        let input = "\"identifier\"";
        let expected = Node::Identifier(Identifier::new("identifier".to_string()));
        let result = Parse::parse_identifier(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "\"another_identifier\"";
        let expected = Node::Identifier(Identifier::new("another_identifier".to_string()));
        let result = Parse::parse_identifier(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "\"12345\"";
        let expected = Node::Identifier(Identifier::new("12345".to_string()));
        let result = Parse::parse_identifier(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "\"special_chars!@#\"";
        let expected = Node::Identifier(Identifier::new("special_chars!@#".to_string()));
        let result = Parse::parse_identifier(input);
        assert_eq!(result, Ok(("", expected)));
    }

    #[test]
    fn test_parse_constant() {
        let input = "constant";
        let expected = Node::Identifier(Identifier::new("constant".to_string()));
        let result = Parse::parse_constant(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "another_constant";
        let expected = Node::Identifier(Identifier::new("another_constant".to_string()));
        let result = Parse::parse_constant(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "12345";
        let expected = Node::Identifier(Identifier::new("12345".to_string()));
        let result = Parse::parse_constant(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "special_chars!@#";
        let expected = Node::Identifier(Identifier::new("special_chars".to_string()));
        let result = Parse::parse_constant(input);
        assert_eq!(result, Ok(("!@#", expected)));
    }

    #[test]
    fn test_parse_command() {
        let input = "echo";
        let expected = Node::Command(Box::new(Command(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![],
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "echo hello";
        let expected = Node::Command(Box::new(Command(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("hello".to_string()))],
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "echo         hello";
        let expected = Node::Command(Box::new(Command(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("hello".to_string()))],
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

        let input = "echo \"だんごむし\"";
        let expected = Node::Command(Box::new(Command(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new(
                "だんごむし".to_string(),
            ))],
        )));
        let result = Parse::parse_command(input);
        assert_eq!(result, Ok(("", expected)));

    }

    #[test]
    fn test_parse_compound_statement() {
        let input = "echo \"aaaa\"; echo \"だんごむし\"";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::Command(Box::new(Command(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new("aaaa".to_string()))],
            ))),
            Node::Command(Box::new(Command(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new(
                    "だんごむし".to_string(),
                ))],
            ))),
        ]));

        let result = Parse::parse_compound_statement(input);
        assert_eq!(result, Ok(("", expected)));
        let input = "echo hello; echo world";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::Command(Box::new(Command(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new("hello".to_string()))],
            ))),
            Node::Command(Box::new(Command(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new("world".to_string()))],
            ))),
        ]));
        let result = Parse::parse_compound_statement(input);
        assert_eq!(result, Ok(("", expected)));
    }
}
