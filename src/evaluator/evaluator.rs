use crate::error::error::{RshError, Status};
use crate::parser::parse::{Command, CompoundStatement, Identifier, Node};
use crate::rsh::rsh::Rsh;
pub struct Evaluator(Rsh);

impl Evaluator {
    pub fn new(rsh: Rsh) -> Self {
        Evaluator(rsh)
    }

    fn eval_identifier(&self, expr: Identifier) {
        println!("Identifier: {:?}", expr.eval());
    }

    fn eval_command(&self, expr: Command) {
        let command = match expr.get_command() {
            Node::Identifier(identifier) => {
                self.eval_identifier(identifier.clone());
                identifier.eval()
            }
            _ => {
                // Provide a default value or handle the case where the command is not an identifier
                Default::default() // Replace with an appropriate default value
            }
        };
        let sub_command = expr
            .get_sub_command()
            .into_iter()
            .map(|node| match node {
                Node::Identifier(identifier) => identifier.eval(),
                _ => Default::default(), // Handle other cases appropriately
            })
            .collect::<Vec<String>>();

        let mut full_command = vec![command];
        full_command.extend(sub_command);

        // 分割したコマンドを実行
        Rsh::new().rsh_execute(full_command);
    }

    fn eval_compound_statement(&self, expr: CompoundStatement) {
        let expr = expr.eval();
        for s in expr {
            match s {
                Node::Command(command) => {
                    self.eval_command(*command);
                }
                _ => {}
            }
        }
    }

    pub fn evaluate(&self, ast: Node) -> Result<Status, RshError> {
        // ASTを評価
        match ast {
            Node::CompoundStatement(stmt) => {
                self.eval_compound_statement(stmt);
            }
            _ => {}
        }
        Ok(Status::Success)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_compound_statement() {
        let evaluator = Evaluator::new();
        let compound_statement = CompoundStatement::new(vec![]); // Adjust with appropriate initialization
        evaluator.eval_compound_statement(compound_statement);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_evaluate_with_compound_statement() {
        let evaluator = Evaluator::new();
        let compound_statement = CompoundStatement::new(vec![]); // Adjust with appropriate initialization
        let ast = Node::CompoundStatement(compound_statement);
        let result = evaluator.evaluate(ast);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Status::Success);
    }

    #[test]
    fn test_evaluate_with_other_node() {
        let evaluator = Evaluator::new();
        let other_node = Node::Identifier(Identifier::new("hello".to_string())); // Replace with an actual variant of Node
        let result = evaluator.evaluate(other_node);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Status::Success);
    }

    #[test]
    fn test_eval_command_with_identifier() {
        let evaluator = Evaluator::new();
        let identifier = Identifier::new("test_command".to_string());
        let command = Command::new(Node::Identifier(identifier.clone()), vec![]);
        evaluator.eval_command(command);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_eval_command_with_sub_commands() {
        let evaluator = Evaluator::new();
        let identifier = Identifier::new("echo".to_string());
        let sub_identifier = Identifier::new("hello, world".to_string());
        let command = Command::new(
            Node::Identifier(identifier.clone()),
            vec![Node::Identifier(sub_identifier.clone())],
        );
        evaluator.eval_command(command);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_eval_command_with_non_identifier() {
        let evaluator = Evaluator::new();
        let non_identifier_node = Node::CompoundStatement(CompoundStatement::new(vec![])); // Replace with an actual non-identifier variant of Node
        let command = Command::new(non_identifier_node, vec![]);
        evaluator.eval_command(command);
        // 期待される動作を確認するためのアサーションを追加
    }
}
