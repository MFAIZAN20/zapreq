use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

/// CAUS-CLI-21:
/// Pretty output mode options.
#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum PrettyMode {
    All,
    Colors,
    Format,
    None,
}

/// CAUS-CLI-21:
/// Syntax theme choices for terminal rendering.
#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum StyleTheme {
    Monokai,
    Solarized,
    Dracula,
    Autumn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum SeverityLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DocFormat {
    Markdown,
    Openapi,
    Html,
}

#[derive(Clone, Debug, Args, Default)]
pub struct SourceSelector {
    #[arg(long = "alias")]
    pub alias: Option<String>,
    #[arg(long = "workspace")]
    pub workspace: Option<String>,
    #[arg(long = "request")]
    pub request: Option<String>,
    #[arg(long = "file")]
    pub file: Option<String>,
}

/// CAUS-PLUGINMGMT-31, CAUS-PLUGINMGMT-35:
/// Plugin manager subcommands.
#[derive(Clone, Debug, Subcommand)]
pub enum PluginCommand {
    Install {
        name: String,
    },
    Uninstall {
        name: String,
    },
    List,
    Validate,
    Run {
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// Environment profile commands.
#[derive(Clone, Debug, Subcommand)]
pub enum EnvCommand {
    List,
    Show { name: String },
    Validate { name: String },
}

/// Secret management commands.
#[derive(Clone, Debug, Subcommand)]
pub enum SecretCommand {
    Set {
        key: String,
        value: String,
    },
    Get {
        key: String,
        #[arg(long = "reveal")]
        reveal: bool,
    },
    List,
}

/// Structured workspace/collection commands.
#[derive(Clone, Debug, Subcommand)]
pub enum CollectionsCommand {
    List,
    New {
        name: String,
    },
    Import {
        name: String,
        path: String,
    },
    Export {
        name: String,
        path: String,
        #[arg(long = "format", default_value = "zapreq")]
        format: String,
    },
    Migrate {
        #[arg(long = "workspace", default_value = "legacy")]
        workspace: String,
    },
}

/// Workspace request commands.
#[derive(Clone, Debug, Subcommand)]
pub enum RequestsCommand {
    List {
        workspace: String,
    },
    Run {
        workspace: String,
        request: String,
        #[arg(long = "env-profile")]
        env_profile: Option<String>,
    },
    Save {
        workspace: String,
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        request: Vec<String>,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum SecurityCommand {
    Scan {
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "severity", default_value = "low")]
        severity: SeverityLevel,
        #[arg(long = "live")]
        live: bool,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum DocsCommand {
    Generate {
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "format", default_value = "markdown")]
        format: DocFormat,
        #[arg(long = "output")]
        output: Option<String>,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum RegressionCommand {
    Add {
        suite: String,
        name: String,
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "expect-status")]
        expect_status: Option<u16>,
        #[arg(long = "expect-header")]
        expect_header: Vec<String>,
        #[arg(long = "expect-json")]
        expect_json: Vec<String>,
        #[arg(long = "expect-body-contains")]
        expect_body_contains: Vec<String>,
        #[arg(long = "max-time-ms")]
        max_time_ms: Option<u64>,
    },
    List {
        #[arg(long = "suite")]
        suite: Option<String>,
    },
    Run {
        suite: String,
    },
    Delete {
        suite: String,
        name: String,
    },
    History {
        suite: String,
        name: String,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum PerfCommand {
    Benchmark {
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "iterations", default_value_t = 5)]
        iterations: u32,
        #[arg(long = "duration-secs")]
        duration_secs: Option<u64>,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum NotesCommand {
    Add {
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        body: String,
    },
    Update {
        id: i64,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        body: String,
    },
    List {
        #[command(flatten)]
        source: SourceSelector,
        #[arg(long = "query")]
        query: Option<String>,
    },
    History {
        id: i64,
    },
}

/// CAUS-PLUGINMGMT-31:
/// Top-level CLI subcommands.
#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    Plugins {
        #[command(subcommand)]
        command: PluginCommand,
    },
    Save {
        alias: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        request: Vec<String>,
    },
    Run {
        alias: String,
        #[arg(long = "env-profile")]
        env_profile: Option<String>,
    },
    List,
    Delete {
        alias: String,
    },
    Ai {
        prompt: String,
        #[arg(long = "send")]
        send: bool,
        #[arg(long = "save")]
        save: Option<String>,
        #[arg(long = "explain")]
        explain: bool,
        #[arg(long = "env-profile")]
        env_profile: Option<String>,
    },
    Test {
        #[arg(long = "expect-status")]
        expect_status: Option<u16>,
        #[arg(long = "expect-header")]
        expect_header: Vec<String>,
        #[arg(long = "expect-json")]
        expect_json: Vec<String>,
        #[arg(long = "expect-body-contains")]
        expect_body_contains: Vec<String>,
        #[arg(long = "max-time-ms")]
        max_time_ms: Option<u64>,
        #[arg(long = "report", default_value = "text")]
        report: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        request: Vec<String>,
    },
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    Collections {
        #[command(subcommand)]
        command: CollectionsCommand,
    },
    Requests {
        #[command(subcommand)]
        command: RequestsCommand,
    },
    Security {
        #[command(subcommand)]
        command: SecurityCommand,
    },
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
    },
    Regression {
        #[command(subcommand)]
        command: RegressionCommand,
    },
    Perf {
        #[command(subcommand)]
        command: PerfCommand,
    },
    Notes {
        #[command(subcommand)]
        command: NotesCommand,
    },
    Secrets {
        #[command(subcommand)]
        command: SecretCommand,
    },
    #[command(alias = "ui")]
    Gui,
    Tui,
    Diff {
        url_a: String,
        url_b: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        request: Vec<String>,
    },
}

/// CAUS-CLI-21, CAUS-CLI-22:
/// Final normalized CLI argument contract.
#[derive(Clone, Debug)]
pub struct CliArgs {
    pub method: String,
    pub url: String,
    pub request_items: Vec<String>,
    pub json: bool,
    pub form: bool,
    pub multipart: bool,
    pub pretty: Option<PrettyMode>,
    pub style: Option<StyleTheme>,
    pub print: Option<String>,
    pub headers: bool,
    pub body: bool,
    pub verbose: bool,
    pub stream: bool,
    pub download: bool,
    pub output: Option<String>,
    pub continue_download: bool,
    pub auth: Option<String>,
    pub auth_type: String,
    pub verify: bool,
    pub ssl: Option<String>,
    pub timeout: Option<f64>,
    pub follow: bool,
    pub max_redirects: Option<usize>,
    pub proxy: Vec<String>,
    pub cert: Option<String>,
    pub cert_key: Option<String>,
    pub check_status: bool,
    pub ignore_stdin: bool,
    pub no_auth_cookie_warning: bool,
    pub default_scheme: String,
    pub session: Option<String>,
    pub session_read_only: bool,
    pub env_file: Option<String>,
    pub env_profile: Option<String>,
    pub offline: bool,
    pub meta: bool,
    pub summary: bool,
    pub no_summary: bool,
    pub command: Option<Command>,
}

/// Public alias for downstream modules that expect `Cli`.
pub type Cli = CliArgs;

/// CAUS-CLI-21:
/// Raw parser struct used to support optional METHOD positional input.
#[derive(Debug, Parser)]
#[command(name = "zapreq")]
#[command(disable_help_subcommand = true)]
#[command(disable_help_flag = true)]
#[command(args_override_self = true)]
pub struct CliArgsRaw {
    /// METHOD or URL (when URL is given as single positional)
    pub method_or_url: Option<String>,

    /// URL when METHOD is present
    pub maybe_url: Option<String>,

    /// REQUEST_ITEMS variadic tokens
    pub request_items: Vec<String>,

    #[arg(short = 'j', long = "json")]
    pub json: bool,

    #[arg(short = 'f', long = "form")]
    pub form: bool,

    #[arg(long = "multipart")]
    pub multipart: bool,

    #[arg(long = "pretty")]
    pub pretty: Option<PrettyMode>,

    #[arg(long = "style")]
    pub style: Option<StyleTheme>,

    #[arg(short = 'p', long = "print")]
    pub print: Option<String>,

    #[arg(short = 'h', long = "headers")]
    pub headers: bool,

    #[arg(short = 'b', long = "body")]
    pub body: bool,

    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    #[arg(short = 's', long = "stream")]
    pub stream: bool,

    #[arg(short = 'd', long = "download")]
    pub download: bool,

    #[arg(short = 'o', long = "output")]
    pub output: Option<String>,

    #[arg(short = 'c', long = "continue")]
    pub continue_download: bool,

    #[arg(short = 'a', long = "auth")]
    pub auth: Option<String>,

    #[arg(long = "auth-type", default_value = "basic")]
    pub auth_type: String,

    #[arg(long = "verify", value_parser = clap::builder::BoolishValueParser::new())]
    pub verify: Option<bool>,

    #[arg(long = "ssl")]
    pub ssl: Option<String>,

    #[arg(long = "timeout")]
    pub timeout: Option<f64>,

    #[arg(long = "follow")]
    pub follow: bool,

    #[arg(long = "max-redirects", default_value_t = 10)]
    pub max_redirects: usize,

    #[arg(long = "proxy")]
    pub proxy: Vec<String>,

    #[arg(long = "cert")]
    pub cert: Option<String>,

    #[arg(long = "cert-key")]
    pub cert_key: Option<String>,

    #[arg(short = 'S', long = "check-status")]
    pub check_status: bool,

    #[arg(long = "ignore-stdin")]
    pub ignore_stdin: bool,

    #[arg(long = "no-auth-cookie-warning")]
    pub no_auth_cookie_warning: bool,

    #[arg(long = "default-scheme", default_value = "https")]
    pub default_scheme: String,

    #[arg(long = "session")]
    pub session: Option<String>,

    #[arg(long = "session-read-only")]
    pub session_read_only: bool,

    #[arg(short = 'e', long = "env")]
    pub env_file: Option<String>,

    #[arg(long = "env-profile")]
    pub env_profile: Option<String>,

    #[arg(long = "offline")]
    pub offline: bool,

    #[arg(long = "meta")]
    pub meta: bool,

    #[arg(long = "summary")]
    pub summary: bool,

    #[arg(long = "no-summary", conflicts_with = "summary")]
    pub no_summary: bool,

    #[arg(long = "help", action = clap::ArgAction::HelpLong)]
    pub help: Option<bool>,
}

#[derive(Debug, Parser)]
#[command(name = "zapreq")]
#[command(disable_help_subcommand = true)]
#[command(args_override_self = true)]
struct CommandOnly {
    #[command(subcommand)]
    command: Command,
}

/// CAUS-CLI-21, CAUS-CLI-25:
/// Parses CLI args from a provided argv vector.
pub fn parse_cli_from<T>(argv: T) -> Result<CliArgs>
where
    T: IntoIterator,
    T::Item: Into<std::ffi::OsString> + Clone,
{
    let argv_vec: Vec<std::ffi::OsString> = argv.into_iter().map(Into::into).collect();

    if is_subcommand_invocation(&argv_vec) {
        let cmd = match CommandOnly::try_parse_from(argv_vec) {
            Ok(parsed) => parsed,
            Err(e) => {
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion
                {
                    e.exit();
                }
                return Err(anyhow!("CLI parse failed: {e}"));
            }
        };
        return Ok(CliArgs {
            method: "GET".to_string(),
            url: String::new(),
            request_items: Vec::new(),
            json: false,
            form: false,
            multipart: false,
            pretty: None,
            style: None,
            print: None,
            headers: false,
            body: false,
            verbose: false,
            stream: false,
            download: false,
            output: None,
            continue_download: false,
            auth: None,
            auth_type: "basic".to_string(),
            verify: true,
            ssl: None,
            timeout: None,
            follow: false,
            max_redirects: Some(10),
            proxy: Vec::new(),
            cert: None,
            cert_key: None,
            check_status: false,
            ignore_stdin: false,
            no_auth_cookie_warning: false,
            default_scheme: "https".to_string(),
            session: None,
            session_read_only: false,
            env_file: None,
            env_profile: None,
            offline: false,
            meta: false,
            summary: false,
            no_summary: false,
            command: Some(cmd.command),
        });
    }

    let raw = match CliArgsRaw::try_parse_from(argv_vec) {
        Ok(parsed) => parsed,
        Err(e) => {
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                e.exit();
            }
            return Err(anyhow!("CLI parse failed: {e}"));
        }
    };

    let first = raw
        .method_or_url
        .ok_or_else(|| anyhow!("missing METHOD/URL positional arguments"))?;

    let (method, url, request_items) = parse_positional_args(first, raw.maybe_url, raw.request_items);

    Ok(CliArgs {
        method,
        url,
        request_items,
        json: raw.json,
        form: raw.form,
        multipart: raw.multipart,
        pretty: raw.pretty,
        style: raw.style,
        print: raw.print,
        headers: raw.headers,
        body: raw.body,
        verbose: raw.verbose,
        stream: raw.stream,
        download: raw.download,
        output: raw.output,
        continue_download: raw.continue_download,
        auth: raw.auth,
        auth_type: raw.auth_type,
        verify: raw.verify.unwrap_or(true),
        ssl: raw.ssl,
        timeout: raw.timeout,
        follow: raw.follow,
        max_redirects: Some(raw.max_redirects),
        proxy: raw.proxy,
        cert: raw.cert,
        cert_key: raw.cert_key,
        check_status: raw.check_status,
        ignore_stdin: raw.ignore_stdin,
        no_auth_cookie_warning: raw.no_auth_cookie_warning,
        default_scheme: raw.default_scheme,
        session: raw.session,
        session_read_only: raw.session_read_only,
        env_file: raw.env_file,
        env_profile: raw.env_profile,
        offline: raw.offline,
        meta: raw.meta,
        summary: raw.summary,
        no_summary: raw.no_summary,
        command: None,
    })
}

/// CAUS-CLI-22:
/// Infers METHOD when omitted. POST is used when body-style items are present.
fn infer_method(items: &[String]) -> String {
    if items.iter().any(|raw| item_implies_body(raw)) {
        "POST".to_string()
    } else {
        "GET".to_string()
    }
}

/// CAUS-CLI-22:
/// Checks whether a request-item token contributes request body data.
fn item_implies_body(raw: &str) -> bool {
    raw.contains(":=@")
        || raw.contains("=@")
        || raw.contains(":=")
        || raw.contains("@")
        || (raw.contains('=') && !raw.contains("=="))
}

fn is_subcommand_invocation(argv: &[std::ffi::OsString]) -> bool {
    let Some(cmd) = argv.get(1).and_then(|s| s.to_str()) else {
        return false;
    };
    is_known_subcommand_name(cmd)
}

fn looks_like_method(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

pub fn is_known_subcommand_name(cmd: &str) -> bool {
    matches!(
        cmd,
        "plugins"
            | "save"
            | "run"
            | "list"
            | "delete"
            | "ai"
            | "test"
            | "env"
            | "collections"
            | "requests"
            | "security"
            | "docs"
            | "regression"
            | "perf"
            | "notes"
            | "secrets"
            | "gui"
            | "ui"
            | "tui"
            | "diff"
    )
}

fn parse_positional_args(
    first: String,
    maybe_url: Option<String>,
    request_items: Vec<String>,
) -> (String, String, Vec<String>) {
    if let Some(second_positional) = maybe_url {
        if looks_like_method(&first) {
            (
                first.to_ascii_uppercase(),
                second_positional,
                request_items,
            )
        } else {
            let mut items = Vec::with_capacity(1 + request_items.len());
            items.push(second_positional);
            items.extend(request_items);
            let inferred = infer_method(&items);
            (inferred, first, items)
        }
    } else {
        let inferred = infer_method(&request_items);
        (inferred, first, request_items)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_known_subcommand_name, parse_cli_from, Command};

    #[test]
    fn ui_alias_is_recognized_as_subcommand() {
        assert!(is_known_subcommand_name("ui"));
        let parsed = parse_cli_from(["zapreq", "ui"]).expect("ui alias should parse");
        assert!(matches!(parsed.command, Some(Command::Gui)));
    }

    #[test]
    fn gui_subcommand_parses() {
        let parsed = parse_cli_from(["zapreq", "gui"]).expect("gui should parse");
        assert!(matches!(parsed.command, Some(Command::Gui)));
    }

    #[test]
    fn tui_subcommand_still_parses() {
        let parsed = parse_cli_from(["zapreq", "tui"]).expect("tui should parse");
        assert!(matches!(parsed.command, Some(Command::Tui)));
    }
}
