use nom::branch::{alt, permutation};
use nom::bytes::complete::{tag, take_while};
use nom::character::complete::{
    alphanumeric0, line_ending, multispace0, multispace1, not_line_ending,
};
use nom::combinator::value;
use nom::combinator::{map, opt};
use nom::error::context;
use nom::multi::{many0, many1};
use nom::sequence::{preceded, terminated};
use nom::IResult;

/// 任意の式を表す
#[derive(Debug, PartialEq, Clone)]
pub enum Node {
    CompoundStatement(CompoundStatement),
    Define(Box<Define>),
    Comment(Comment),
    CommandStatement(Box<CommandStatement>),
    Pipeline(Pipeline),
    RedirectInput(Box<RedirectInput>),
    RedirectOutput(Box<RedirectOutput>),
    RedirectErrorOutput(Box<RedirectErrorOutput>),
    Redirect(Box<Redirect>),
    ExecScript(Box<ExecScript>),
    Identifier(Identifier),
}

impl Default for Node {
    fn default() -> Self {
        Node::Identifier(Identifier::new(String::new()))
    }
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

    pub fn get_lhs(&self) -> Node {
        match self {
            Node::CommandStatement(command) => command.get_command(),
            _ => Default::default(),
        }
    }

    pub fn get_rhs(&self) -> Vec<Node> {
        match self {
            Node::CommandStatement(command) => command.get_sub_command(),
            _ => Default::default(),
        }
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

#[derive(Debug, PartialEq, Clone)]
pub struct RedirectInput {
    destination: Node,
}
impl RedirectInput {
    pub fn new(destination: Node) -> RedirectInput {
        RedirectInput {
            destination: destination,
        }
    }

    pub fn get_destination(&self) -> Node {
        self.destination.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct RedirectOutput {
    destination: Node,
}
impl RedirectOutput {
    pub fn new(destination: Node) -> RedirectOutput {
        RedirectOutput {
            destination: destination,
        }
    }

    pub fn get_destination(&self) -> Node {
        self.destination.clone()
    }
}
#[derive(Debug, PartialEq, Clone)]
pub struct RedirectErrorOutput {
    destination: Node,
}
impl RedirectErrorOutput {
    pub fn new(destination: Node) -> RedirectErrorOutput {
        RedirectErrorOutput {
            destination: destination,
        }
    }

    pub fn get_destination(&self) -> Node {
        self.destination.clone()
    }
}


#[derive(Debug, PartialEq, Clone)]
pub struct Redirect {
    command: Node,
    destination: Vec<Node>,
}
impl Redirect {
    pub fn new(command: Node, destination: Vec<Node>) -> Redirect {
        Redirect {
            command: command,
            destination: destination,
        }
    }

    pub fn get_destination(&self) -> Vec<Node> {
        self.destination.clone()
    }

    pub fn get_command(&self) -> Node {
        self.command.clone()
    }
}

// 文字列を表す
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

// パイプ --------------------------------------------------------------------
// command1 | command2  # command1の標準出力をcommand2の標準入力に渡す

// 入力
// command < file   # ファイルの内容をコマンドの標準入力に渡す

// 出力 -------------------------------------------------------------------------
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
// ---------------------------------------------------------------------------

// ----------------------------------------------------------------------------

// メタ文字 -------------------------------------------------------------------
// *	ファイル名マッチで0文字以上の任意文字列にマッチ
// ?	ファイル名マッチで1文字の任意文字にマッチ
// ~	ホームディレクトリ
// #	コメント
// \	メタ文字を無効化 (\メタ文字)
// $	変数展開($FOO)
// "	文字列("...$FOO..." では変数展開が行われる)
// '	文字列('...$FOO...' では変数展開が行われない)
// `	コマンド実行結果参照(`cmd`)
// !	ヒストリ参照 (!number)
// ;	コマンド区切り文字(cmd1 ; cmd2)
// |	コマンドのの実行結果を次のコマンドに渡す(cmd1 | cmd2)
// <	リダイレクト受信(cmd < file)
// >	リダイレクト送信(cmd > file)
// &	コマンドをバックグランド実行(cmd &)
// ( )	コマンドをグループ化((cmd1; cmd2))
// [ ]	if文等で使用するテストコマンド
// { }	変数展開 (${FOO})
// ----------------------------------------------------------------------------

pub struct Parse {}
impl Parse {
    fn parse_comment(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(preceded(tag("#"), not_line_ending), |parsed: &str| {
            Node::Comment(Comment::new(parsed.to_string()))
        })(input)?;
        Ok((no_used, parsed))
    }

    fn parse_constant(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = nom::bytes::complete::is_not("\n \\<>;|=#")(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_identifier(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = context(
            "parse_identifier",
            alt((
                nom::sequence::delimited(tag("\""), nom::bytes::complete::is_not("\""), tag("\"")),
                nom::sequence::delimited(tag("'"), nom::bytes::complete::is_not("'"), tag("'")),
            )),
        )(input)?;
        Ok((
            no_used,
            Node::Identifier(Identifier::new(parsed.to_string())),
        ))
    }

    fn parse_not_space(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = nom::bytes::complete::is_not("\n \\<>;|=#!\"$")(input)?;
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
                    take_while(|c: char| c == ' '),
                    alt((
                        Self::parse_identifier, // "に囲まれている文字列
                        Self::parse_constant,
                    )),
                )))),
            )),
            |(command, options)| {
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
                many1(map(
                    permutation((
                        nom::character::complete::space0,
                        opt(permutation((
                            tag("\\"),
                            opt(permutation((tag("  "), multispace0, Self::parse_comment))),
                            nom::character::complete::space0,
                            opt(tag("\n")),
                        ))),
                        nom::character::complete::space0,
                        alt((
                            Self::parse_identifier, // "に囲まれている文字列
                            Self::parse_constant,
                        )),
                    )),
                    |(_, _, _, sub_command)| sub_command,
                )),
            )),
            |(command, options)| {
                Node::CommandStatement(Box::new(CommandStatement::new(command, options)))
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

    fn parse_redirect_specifier(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = context(
            "parse_redirect_specifier",
            map(
                permutation((
                    multispace0,
                    alt((tag("<"), tag(">"), tag("2>"))),
                    multispace0,
                    Self::parse_filename,
                )),
                |(_, kind, _, filename)| match kind {
                    "<" => Node::RedirectInput(Box::new(RedirectInput::new(filename))),
                    ">" => Node::RedirectOutput(Box::new(RedirectOutput::new(filename))),
                    "2>" => Node::RedirectErrorOutput(Box::new(RedirectErrorOutput::new(filename))),
                    _ => unreachable!(),
                },
            ),
        )(input)
        .map_err(|e| {
            //println!("parse_redirect_specifier error: {:?}", e);
            e
        })?;
        Ok((no_used, parsed))
    }
    fn parse_redirect(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = map(
            permutation((Self::parse_command, many1(Self::parse_redirect_specifier))),
            |(command, destination)| Node::Redirect(Box::new(Redirect::new(command, destination))),
        )(input)?;
        Ok((no_used, parsed))
    }

    //cat test.txt |  sort > sorted.txt
    fn parse_pipeline(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = context(
            "parse_pipeline",
            map(
                permutation((
                    alt((Self::parse_command, Self::parse_redirect)),
                    many1(permutation((
                        multispace0,
                        tag("|"),
                        multispace0,
                        alt((Self::parse_redirect, Self::parse_command)),
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
            ),
        )(input)
        .map_err(|e| e)?;
        Ok((no_used, parsed))
    }
    fn parse_statement(input: &str) -> IResult<&str, Node> {
        let (no_used, parsed) = permutation((
            multispace0,
            alt((
                Self::parse_comment,
                Self::parse_redirect,
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
            alt(
                (
                    many1(map(
                        permutation((Self::parse_statement, opt(tag(";")))),
                        |(stmt, _)| stmt,
                    )),
                    many1(Self::parse_statement), // 改行で終わる
                ), // 改行で終わる
            ),
            |compound_statements| {
                Node::CompoundStatement(CompoundStatement::new(compound_statements))
            },
        )(input)?;
        Ok((no_used, parsed))
    }

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

        let input = "'identi fier'";
        let expected = Node::Identifier(Identifier::new("identi fier".to_string()));
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
        let input = "echo arg1 \\\narg2";
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

        let input = "echo arg1 \\  #comment\n     arg2 \\\n         arg3\n";
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
    fn parse_redirect_input() {
        let input = " < file";
        let expected = Node::RedirectInput(Box::new(RedirectInput::new(Node::Identifier(
            Identifier::new("file".to_string()),
        ))));
        let result = Parse::parse_redirect_specifier(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "cmd < file1 < file2";
        let expected = Node::Redirect(Box::new(Redirect::new(
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd".to_string())),
                vec![],
            ))),
            vec![
                Node::RedirectInput(Box::new(RedirectInput::new(Node::Identifier(
                    Identifier::new("file1".to_string()),
                )))),
                Node::RedirectInput(Box::new(RedirectInput::new(Node::Identifier(
                    Identifier::new("file2".to_string()),
                )))),
            ],
        )));
        let result = Parse::parse_redirect(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn parse_redirect_output() {
        let input = " > file";
        let expected = Node::RedirectOutput(Box::new(RedirectOutput::new(Node::Identifier(
            Identifier::new("file".to_string()),
        ))));
        let result = Parse::parse_redirect_specifier(input).unwrap().1;
        assert_eq!(result, expected);

        let input = "cmd > file1 > file2";
        let expected = Node::Redirect(Box::new(Redirect::new(
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd".to_string())),
                vec![],
            ))),
            vec![
                Node::RedirectOutput(Box::new(RedirectOutput::new(Node::Identifier(
                    Identifier::new("file1".to_string()),
                )))),
                Node::RedirectOutput(Box::new(RedirectOutput::new(Node::Identifier(
                    Identifier::new("file2".to_string()),
                )))),
            ],
        )));
        let result = Parse::parse_redirect(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn parse_redirect_combined() {
        let input = "cmd < input > output";
        let expected = Node::Redirect(Box::new(Redirect::new(
            Node::CommandStatement(Box::new(CommandStatement::new(
                Node::Identifier(Identifier::new("cmd".to_string())),
                vec![],
            ))),
            vec![
                Node::RedirectInput(Box::new(RedirectInput::new(Node::Identifier(
                    Identifier::new("input".to_string()),
                )))),
                Node::RedirectOutput(Box::new(RedirectOutput::new(Node::Identifier(
                    Identifier::new("output".to_string()),
                )))),
            ],
        )));
        let result = Parse::parse_redirect(input).unwrap().1;
        assert_eq!(result, expected);
    }
    #[test]
    fn test_parse_complex_command() {
        let input = "echo \"Hello, World!\" > sorted.txt | cat sorted.txt";
        let expected = Node::CompoundStatement(CompoundStatement::new(vec![Node::Pipeline(
            Pipeline::new(vec![
                Node::Redirect(Box::new(Redirect::new(
                    Node::CommandStatement(Box::new(CommandStatement::new(
                        Node::Identifier(Identifier::new("echo".to_string())),
                        vec![Node::Identifier(Identifier::new(
                            "Hello, World!".to_string(),
                        ))],
                    ))),
                    vec![Node::RedirectOutput(Box::new(RedirectOutput::new(
                        Node::Identifier(Identifier::new("sorted.txt".to_string())),
                    )))],
                ))),
                Node::CommandStatement(Box::new(CommandStatement::new(
                    Node::Identifier(Identifier::new("cat".to_string())),
                    vec![Node::Identifier(Identifier::new("sorted.txt".to_string()))],
                ))),
            ]),
        )]));
        let result = Parse::parse_compound_statement(input).unwrap().1;
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
        let input = "echo\ncommand";
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

        let input = "echo arg1;command arg1 arg2;";
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
