use nom::branch::{alt, permutation};
use nom::bytes::complete::{tag, take_while};
use nom::character::complete::{line_ending, multispace0, multispace1, not_line_ending};
use nom::combinator::value;
use nom::combinator::{map, opt};
use nom::multi::{many0, many1};
use nom::sequence::{preceded, terminated};
use nom::IResult;

/// 任意の式を表す
#[derive(Debug, PartialEq, Clone)]
pub enum Node {
    Expression(Box<Node>),
    CompoundStatement(CompoundStatement),
    Statement(Statement),
    Define(Box<Define>),
    Comment(Comment),
    CommandStatement(Box<CommandStatement>),
    Pipeline(Pipeline),
    ExecScript(Box<ExecScript>),
    Identifier(Identifier),
}
impl Node {
    /// 式を評価する
    pub fn eval(&self) -> Vec<Node> {
        match self {
            Node::CompoundStatement(compound_statement) => compound_statement.eval(),
            Node::CommandStatement(command) => command.0.eval(),
            Node::Pipeline(pipeline) => pipeline.get_commands(),
            _ => Default::default(),
        }
    }

    pub fn get_node(&self) -> Node {
        self.clone()
    }

    pub fn get_sub_command(&self) -> Vec<Node> {
        match self {
            Node::CommandStatement(command) => command.get_sub_command(),
            _ => Default::default(),
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
pub struct Define {
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
pub struct CommandStatement(Node, Vec<Node>);
impl CommandStatement {
    // メインコマンド・引数のセット
    pub fn new(val: Node, val2: Vec<Node>) -> CommandStatement {
        CommandStatement(val, val2)
    }

    // メインコマンドを返す
    pub fn get_command(&self) -> Node {
        self.0.clone()
    }

    // コマンド引数を返す
    pub fn get_sub_command(&self) -> Vec<Node> {
        self.1.clone()
    }
}
#[derive(Debug, PartialEq, Clone)]
pub struct Pipeline(Vec<Node>);
impl Pipeline {
    pub fn new(val: Vec<Node>) -> Pipeline {
        Pipeline(val)
    }
    pub fn from(val: Node) -> Pipeline {
        Pipeline(Vec::from([val]))
    }
    pub fn get_commands(&self) -> Vec<Node> {
        self.0.clone()
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

// 一行のコメントを表す
#[derive(Debug, PartialEq, Clone)]
pub struct Comment {
    comment: String,
}
impl Comment {
    pub fn new(val: String) -> Comment {
        Comment { comment: val }
    }
    pub fn get_comment(&self) -> String {
        self.comment.clone()
    }
}

// 実行可能ファイルやスクリプトの実行を表す
#[derive(Debug, PartialEq, Clone)]
pub struct ExecScript {
    exec_script: Node,
}
impl ExecScript {
    pub fn new(val: Node) -> ExecScript {
        ExecScript { exec_script: val }
    }
    pub fn get_filename(&self) -> Node {
        self.exec_script.clone()
    }
}

// パイプ
// command1 | command2  # command1の標準出力をcommand2の標準入力に渡す

// 入力
// command < file   # ファイルの内容をコマンドの標準入力に渡す

// 出力
// command >&2      # 標準出力を標準エラー出力にリダイレクト

// command > file   # ファイル作成 or 上書き
// command >> file  # 追加出力。ファイルがなければ作成
// command 2> file  # 標準エラー出力をファイルにリダイレクト(作成 or 上書き)

// command &> file      # 標準出力/エラー出力を同一ファイルにリダイレクト
// command > file 2>&1  # 同上
// command &>> file     # 標準出力/エラー出力を同一ファイルに追加書き込み
// command >> file 2>&1 # 同上

// command > file1 2> file2   # 標準出力,エラー出力を別々のファイルにリダイレクト
// command >> file1 2>> file2 # 標準出力,エラー出力を別々のファイルに追加書き込み

pub struct Parse {}
impl Parse {
    fn parse_comment(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(preceded(tag("#"), not_line_ending), |parsed: &str| {
            Node::Comment(Comment::new(parsed.to_string()))
        })(input)?;
        Ok((no_used, parsed))
    }

    fn parse_constant(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = nom::bytes::complete::is_not("\n \\|=#")(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_identifier(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = alt((
            nom::sequence::delimited(tag("\""), nom::bytes::complete::is_not("\""), tag("\"")),
            nom::sequence::delimited(tag("'"), nom::bytes::complete::is_not("'"), tag("'")),
        ))(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_not_space(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = nom::bytes::complete::is_not(" ")(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_filename_with_dot(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            many1(alt((tag("."), nom::character::complete::alphanumeric1))),
            |parsed| {
                let mut s = String::new();
                for p in parsed {
                    s.push_str(p);
                }
                Node::Identifier(Identifier::new(s))
            },
        )(input)?;
        Ok((no_used, parsed))
    }

    fn parse_filename(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = alt((
            Self::parse_filename_with_dot, /* 拡張子が含まれる */
            Self::parse_not_space,         /* 拡張子が含まれない */
        ))(input)?;

        Ok((no_used, parsed))
    }

    fn parse_exec_script(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = permutation((tag("./"), Self::parse_filename))(input)?;

        Ok((
            no_used,
            Node::ExecScript(Box::new(ExecScript::new(parsed.1))),
        ))
    }

    fn parse_command(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                Self::parse_constant,
                opt(many1(permutation((
                    multispace0,
                    alt((
                        Self::parse_identifier, // "に囲まれている文字列
                        Self::parse_constant,
                    )),
                )))),
                opt(line_ending),
            )),
            |(command, options, _)| {
                if let Some(options) = options {
                    let mut v: Vec<Node> = Vec::new();

                    for opt in options {
                        v.push(opt.1.clone());
                    }
                    Node::CommandStatement(Box::new(CommandStatement::new(command, v)))
                } else {
                    Node::CommandStatement(Box::new(CommandStatement::new(command, Vec::new())))
                }
            },
        )(input)?;

        Ok((no_used, parsed))
    }

    fn parse_command_with_backslash(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                Self::parse_constant,
                many1(permutation((
                    multispace0,
                    opt(tag("\\")),
                    multispace0,
                    alt((
                        Self::parse_identifier, // "に囲まれている文字列
                        Self::parse_constant,
                    )),
                    multispace0,
                    opt(tag("\\")),
                ))),
                line_ending,
            )),
            |(command, options, _)| {
                let mut v: Vec<Node> = Vec::new();

                for opt in options {
                    v.push(opt.3.clone());
                }
                Node::CommandStatement(Box::new(CommandStatement::new(command, v)))
            },
        )(input)?;

        Ok((no_used, parsed))
    }

    fn parse_define(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                multispace0,
                Self::parse_constant,
                tag("="),
                Self::parse_identifier,
            )),
            |(_, var, _, data)| Node::Define(Box::new(Define::new(var, data))),
        )(input)?;
        Ok((no_used, parsed))
    }

    fn parse_pipeline(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((
                Self::parse_command,
                many1(permutation((
                    multispace0,
                    tag("|"),
                    multispace0,
                    Self::parse_command,
                ))),
            )),
            |(command, options)| {
                let mut v: Vec<Node> = Vec::new();
                v.push(command);
                for opt in options {
                    v.push(opt.3.clone());
                }
                Node::Pipeline(Pipeline::new(v))
            },
        )(input)?;
        Ok((no_used, parsed))
    }

    fn parse_statement(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = permutation((
            multispace0,
            alt((
                Self::parse_comment,
                Self::parse_exec_script,
                Self::parse_define,
                Self::parse_pipeline,
                Self::parse_command_with_backslash,
                Self::parse_command,
            )),
            multispace0,
        ))(input)?;
        Ok((no_used, parsed.1))
    }

    fn parse_compound_statement(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            many1(Self::parse_statement), // 単数コマンド
            |compound_statements| {
                Node::CompoundStatement(CompoundStatement::new(compound_statements))
            },
        )(input)?;
        Ok((no_used, parsed))
    }
    //many1(terminated(Self::parse_statement, line_ending)), // 改行で終わる

    pub fn parse_node(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = Self::parse_compound_statement(input)?;
        Ok((no_used, parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_pipeline() {
        let input = "command1 | command2";
        let expected = Node::Pipeline(Pipeline::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command1".to_string())),
                vec![],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command2".to_string())),
                vec![],
            ))),
        ]));
        let result = Parse::parse_pipeline(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "command1 arg1 | command2 arg2";
        let expected = Node::Pipeline(Pipeline::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command1".to_string())),
                vec![Node::Identifier(Identifier::new("arg1".to_string()))],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command2".to_string())),
                vec![Node::Identifier(Identifier::new("arg2".to_string()))],
            ))),
        ]));
        let result = Parse::parse_pipeline(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_filename() {
        let input = "filename";
        let expected = Node::Identifier(Identifier::new("filename".to_string()));
        let result = Parse::parse_filename(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "file.name";
        let expected = Node::Identifier(Identifier::new("file.name".to_string()));
        let result = Parse::parse_filename(input).unwrap().1;
        assert_eq!(result, expected);

        let input = ".configfile";
        let expected = Node::Identifier(Identifier::new(".configfile".to_string()));
        let result = Parse::parse_filename(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "特殊な文字列のファイル名.txt";
        let expected =
            Node::Identifier(Identifier::new("特殊な文字列のファイル名.txt".to_string()));
        let result = Parse::parse_filename(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_constant() {
        let input = "constant";
        let expected = Node::Identifier(Identifier::new("constant".to_string()));
        let result = Parse::parse_constant(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_identifier() {
        let input = "\"identifier\"";
        let expected = Node::Identifier(Identifier::new("identifier".to_string()));
        let result = Parse::parse_identifier(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "'identifier'";
        let expected = Node::Identifier(Identifier::new("identifier".to_string()));
        let result = Parse::parse_identifier(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_not_space() {
        let input = "not_space";
        let expected = Node::Identifier(Identifier::new("not_space".to_string()));
        let result = Parse::parse_not_space(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_filename_with_dot() {
        let input = "file.name";
        let expected = Node::Identifier(Identifier::new("file.name".to_string()));
        let result = Parse::parse_filename_with_dot(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_exec_script() {
        let input = "./script.sh";
        let expected = Node::ExecScript(Box::new(ExecScript::new(Node::Identifier(
            Identifier::new("script.sh".to_string()),
        ))));
        let result = Parse::parse_exec_script(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_command() {
        let input = "command arg1 arg2\n";
        let expected = Node::CommandStatement(Box::new(CommandStatement::new(
            Node::Identifier(Identifier::new("command".to_string())),
            vec![
                Node::Identifier(Identifier::new("arg1".to_string())),
                Node::Identifier(Identifier::new("arg2".to_string())),
            ],
        )));
        let result = Parse::parse_command(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "echo arg1";
        let expected = Node::CommandStatement(Box::new(CommandStatement::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("arg1".to_string()))],
        )));
        let result = Parse::parse_command(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_command_with_backslash() {
        let input = "echo arg1 \\ arg2";
        let expected = Node::CommandStatement(Box::new(CommandStatement::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![
                Node::Identifier(Identifier::new("arg1".to_string())),
                Node::Identifier(Identifier::new("arg2".to_string())),
            ],
        )));
        let result = Parse::parse_command_with_backslash(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "echo arg1 \\ arg2 \\ arg3\n";
        let expected = Node::CommandStatement(Box::new(CommandStatement::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![
                Node::Identifier(Identifier::new("arg1".to_string())),
                Node::Identifier(Identifier::new("arg2".to_string())),
                Node::Identifier(Identifier::new("arg3".to_string())),
            ],
        )));
        let result = Parse::parse_command_with_backslash(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "echo arg1 \\\n     arg2 \\\n         arg3\n";
        let expected = Node::CommandStatement(Box::new(CommandStatement::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![
                Node::Identifier(Identifier::new("arg1".to_string())),
                Node::Identifier(Identifier::new("arg2".to_string())),
                Node::Identifier(Identifier::new("arg3".to_string())),
            ],
        )));
        let result = Parse::parse_command_with_backslash(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_define() {
        let input = "var=\"value\"";
        let expected = Node::Define(Box::new(Define::new(
            Node::Identifier(Identifier::new("var".to_string())),
            Node::Identifier(Identifier::new("value".to_string())),
        )));
        let result = Parse::parse_define(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_comment() {
        let input = "# comment";
        let expected = Node::Comment(Comment::new(" comment".to_string()));
        let result = Parse::parse_comment(input).unwrap().1;
        assert_eq!(result, expected);
        let input = "# comment\necho ok\n";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::Comment(Comment::new(" comment".to_string())),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new("ok".to_string()))],
            ))),
        ]));
        let result = Parse::parse_compound_statement(input).unwrap().1;
        assert_eq!(result, expected);
    }
    #[test]
    fn parse_pipeline() {
        let input = "cmd1 | cmd2 | cmd3";
        let expected = Node::Pipeline(Pipeline::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd1".to_string())),
                vec![],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd2".to_string())),
                vec![],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd3".to_string())),
                vec![],
            ))),
        ]));
        let result = Parse::parse_pipeline(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "cmd1 arg1 | cmd2 arg2 | cmd3 arg3";
        let expected = Node::Pipeline(Pipeline::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd1".to_string())),
                vec![Node::Identifier(Identifier::new("arg1".to_string()))],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd2".to_string())),
                vec![Node::Identifier(Identifier::new("arg2".to_string()))],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd3".to_string())),
                vec![Node::Identifier(Identifier::new("arg3".to_string()))],
            ))),
        ]));
        let result = Parse::parse_pipeline(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_statement() {
        let input = "var=\"value\"";
        let expected = Node::Define(Box::new(Define::new(
            Node::Identifier(Identifier::new("var".to_string())),
            Node::Identifier(Identifier::new("value".to_string())),
        )));
        let result = Parse::parse_statement(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_compound_statement() {
        let input = "echo\\\ncommand\n";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command".to_string())),
                vec![],
            ))),
        ]));
        let result = Parse::parse_compound_statement(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "echo arg1\ncommand arg1 arg2\n";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("echo".to_string())),
                vec![Node::Identifier(Identifier::new("arg1".to_string()))],
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command".to_string())),
                vec![
                    Node::Identifier(Identifier::new("arg1".to_string())),
                    Node::Identifier(Identifier::new("arg2".to_string())),
                ],
            ))),
        ]));
        let result = Parse::parse_compound_statement(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "var=\"value\"\ncommand arg1 arg2\n";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![
            Node::Define(Box::new(Define::new(
                Node::Identifier(Identifier::new("var".to_string())),
                Node::Identifier(Identifier::new("value".to_string())),
            ))),
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("command".to_string())),
                vec![
                    Node::Identifier(Identifier::new("arg1".to_string())),
                    Node::Identifier(Identifier::new("arg2".to_string())),
                ],
            ))),
        ]));
        let result = Parse::parse_compound_statement(input).unwrap().1;
        assert_eq!(result, expected);
    }
}
