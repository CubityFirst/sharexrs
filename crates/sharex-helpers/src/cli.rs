//! `ShareX.HelpersLib/CLI`: `CLIManager` and `CLICommand`, how ShareX reads
//! its command line, quirks included:
//!
//! - only `-` starts a command (`-silent`); `/silent` and `--silent` are not
//!   ShareX's `silent` (the second is a command named `-silent`);
//! - the argument after a command is its parameter unless it starts with
//!   `-`, so `-silent file.png` gives `-silent` the parameter `file.png`
//!   instead of uploading the file;
//! - names compare ordinally, ignoring case;
//! - an empty argument stops the parsing (`argument[0]` throws, and
//!   `ParseCommands` catches it): the commands before it are kept.

use std::fmt;

use crate::culture::ordinal_ignore_case_eq;

/// `CLICommand`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliCommand {
    /// The name without its `-`, or a whole argument that isn't a command.
    pub command: String,
    pub parameter: Option<String>,
    /// Started with `-`.
    pub is_command: bool,
}

impl CliCommand {
    /// `CLICommand.CheckCommand`: ordinal, ignoring case.
    pub fn check_command(&self, command: &str) -> bool {
        !self.command.is_empty() && ordinal_ignore_case_eq(&self.command, command)
    }

    /// The parameter, when it isn't empty (`!string.IsNullOrEmpty`).
    pub fn parameter(&self) -> Option<&str> {
        self.parameter.as_deref().filter(|p| !p.is_empty())
    }
}

/// `CLICommand.ToString`: `-name "parameter"`.
impl fmt::Display for CliCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_command {
            f.write_str("-")?;
        }
        f.write_str(&self.command)?;
        if let Some(p) = self.parameter() {
            write!(f, " \"{p}\"")?;
        }
        Ok(())
    }
}

/// `CLIManager`.
#[derive(Debug, Clone, Default)]
pub struct CliManager {
    pub arguments: Vec<String>,
    pub commands: Vec<CliCommand>,
}

impl CliManager {
    /// `new CLIManager(string[] arguments)`.
    pub fn new(arguments: &[String]) -> Self {
        Self { arguments: arguments.to_vec(), commands: Vec::new() }
    }

    /// `new CLIManager(string arguments)`: split by [`parse_cli`].
    pub fn from_command_line(arguments: &str) -> Self {
        Self { arguments: parse_cli(arguments), commands: Vec::new() }
    }

    /// `new ShareXCLIManager(args)` then `ParseCommands()`.
    pub fn parse(arguments: &[String]) -> Self {
        let mut cli = Self::new(arguments);
        cli.parse_commands();
        cli
    }

    /// `CLIManager.ParseCommands`. False when an empty argument stopped it
    /// (ShareX's `IndexOutOfRangeException`, written to the log).
    pub fn parse_commands(&mut self) -> bool {
        // The command waiting for its parameter (`lastCommand`).
        let mut last: Option<usize> = None;
        for argument in &self.arguments {
            let Some(first) = argument.chars().next() else {
                return false;
            };
            match last {
                Some(i) if first != '-' => {
                    self.commands[i].parameter = Some(argument.clone());
                    last = None;
                }
                _ => {
                    let mut command = CliCommand::default();
                    if first == '-' {
                        command.is_command = true;
                        command.command = argument[1..].to_owned();
                        last = Some(self.commands.len());
                    } else {
                        command.command = argument.clone();
                    }
                    self.commands.push(command);
                }
            }
        }
        true
    }

    /// `CLIManager.IsCommandExist`: any of `commands` (a leading `-` is
    /// dropped from each) given as a command.
    pub fn is_command_exist(&self, commands: &[&str]) -> bool {
        commands.iter().filter(|c| !c.is_empty()).any(|c| {
            let name = c.strip_prefix('-').unwrap_or(c);
            self.commands.iter().any(|x| x.is_command && x.check_command(name))
        })
    }

    /// `CLIManager.GetCommand`: the first argument named `command`, command
    /// or not.
    pub fn get_command(&self, command: &str) -> Option<&CliCommand> {
        self.commands.iter().find(|x| x.check_command(command))
    }

    /// `CLIManager.GetParameter`.
    pub fn get_parameter(&self, command: &str) -> Option<&str> {
        self.get_command(command).and_then(|c| c.parameter.as_deref())
    }
}

/// `CLIManager.ToString`: one command per line.
impl fmt::Display for CliManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.commands.iter().enumerate() {
            if i > 0 {
                f.write_str("\r\n")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// `CLIManager.ParseCLI`: splits at white space outside double quotes. As
/// ShareX's, a quote opens wherever it is (dropping what came before it
/// in that token), and the last token is only kept when white space or a
/// closing quote ends it.
pub fn parse_cli(arguments: &str) -> Vec<String> {
    let chars: Vec<char> = arguments.chars().collect();
    let mut commands = Vec::new();
    let mut in_quotes = false;
    let mut start = 0;
    for i in 0..chars.len() {
        let c = chars[i];
        if (!in_quotes && c.is_whitespace()) || (in_quotes && c == '"') {
            if i > start {
                commands.push(chars[start..i].iter().collect());
            }
            in_quotes = false;
            start = i + 1;
        } else if c == '"' {
            in_quotes = true;
            start = i + 1;
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn cmd(command: &str, parameter: Option<&str>, is_command: bool) -> CliCommand {
        CliCommand { command: command.into(), parameter: parameter.map(Into::into), is_command }
    }

    #[test]
    fn commands_and_parameters() {
        let cli = CliManager::parse(&args(&["-silent", "-workflow", "My task", "C:\\a.png", "https://x.y/z", "-ExitShareX"]));
        assert_eq!(
            cli.commands,
            vec![
                cmd("silent", None, true),
                cmd("workflow", Some("My task"), true),
                cmd("C:\\a.png", None, false),
                cmd("https://x.y/z", None, false),
                cmd("ExitShareX", None, true),
            ]
        );
        assert!(cli.is_command_exist(&["SILENT"]));
        assert!(cli.is_command_exist(&["-silent"]));
        assert!(cli.is_command_exist(&["s", "silent"]));
        assert!(!cli.is_command_exist(&["s"]));
        assert_eq!(cli.get_parameter("WORKFLOW"), Some("My task"));
        assert_eq!(cli.to_string(), "-silent\r\n-workflow \"My task\"\r\nC:\\a.png\r\nhttps://x.y/z\r\n-ExitShareX");
    }

    #[test]
    fn a_file_after_a_command_is_its_parameter() {
        let cli = CliManager::parse(&args(&["-silent", "a.png", "b.png"]));
        assert_eq!(cli.commands, vec![cmd("silent", Some("a.png"), true), cmd("b.png", None, false)]);
    }

    #[test]
    fn only_a_hyphen_starts_a_command() {
        let cli = CliManager::parse(&args(&["/portable", "--silent", "-p"]));
        assert_eq!(cli.commands, vec![cmd("/portable", None, false), cmd("-silent", None, true), cmd("p", None, true)]);
        assert!(!cli.is_command_exist(&["portable"]));
        assert!(!cli.is_command_exist(&["silent"]));
        // A leading "-" of the name asked for is dropped, so "--silent" asks for "-silent".
        assert!(cli.is_command_exist(&["--silent"]));
        assert!(cli.is_command_exist(&["p"]));
        // Not a command, but GetCommand finds it.
        assert_eq!(cli.get_command("/PORTABLE").map(|c| c.is_command), Some(false));
    }

    #[test]
    fn an_empty_argument_stops_parsing() {
        let mut cli = CliManager::new(&args(&["-silent", "", "-portable"]));
        assert!(!cli.parse_commands());
        assert_eq!(cli.commands, vec![cmd("silent", None, true)]);
        // A lone "-" is a command with no name, which nothing matches.
        let cli = CliManager::parse(&args(&["-", "x"]));
        assert_eq!(cli.commands, vec![cmd("", Some("x"), true)]);
        assert!(!cli.is_command_exist(&[""]));
    }

    #[test]
    fn case_is_ignored_ordinally() {
        let cli = CliManager::parse(&args(&["-ÉCRAN"]));
        assert!(cli.is_command_exist(&["écran"]));
    }

    #[test]
    fn command_line_strings() {
        assert_eq!(parse_cli("-a \"b c\" d "), args(&["-a", "b c", "d"]));
        // The last token needs something to end it.
        assert_eq!(parse_cli("-a b"), args(&["-a"]));
        assert_eq!(parse_cli("-a \"b"), args(&["-a"]));
        // A quote opens mid-token, dropping what came before it.
        assert_eq!(parse_cli("ab\"c d\" "), args(&["c d"]));
        assert_eq!(parse_cli("  \"\"  x\ty\n"), args(&["x", "y"]));
    }
}
