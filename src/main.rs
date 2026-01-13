//! Streamdown - A streaming markdown renderer for modern terminals.
//!
//! This binary provides the CLI interface to the streamdown library,
//! supporting streaming input from files, stdin, or wrapped programs.

mod cli;
mod pty;

use clap::Parser as ClapParser;
use cli::Cli;
use log::{debug, error, info, trace, LevelFilter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use streamdown_config::{ComputedStyle, Config};
use streamdown_parser::{ParseEvent, Parser as MarkdownParser};
use streamdown_plugin::PluginManager;
use streamdown_render::{RenderFeatures, RenderStyle, Renderer};

fn main() {
    let cli = <Cli as ClapParser>::parse();

    // Handle --paths flag
    if cli.show_paths {
        cli::show_paths();
        return;
    }

    // Set up logging
    setup_logging(&cli.log_level);
    info!("Streamdown v{}", env!("CARGO_PKG_VERSION"));

    // Run the main application
    if let Err(e) = run(&cli) {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Set up logging based on the log level argument.
fn setup_logging(level: &str) {
    let filter = match level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Warn,
    };

    env_logger::Builder::new()
        .filter_level(filter)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}

/// Main application logic.
fn run(cli: &Cli) -> io::Result<()> {
    // Load and merge configuration
    let config = load_config(cli)?;
    let computed_style = config.computed_style();
    debug!("Loaded config with style: {:?}", computed_style);

    // Create render features
    let features = create_features(cli);
    debug!("Render features: {:?}", features);

    // Determine input source and process
    if let Some(ref exec_cmd) = cli.exec_cmd {
        // Wrap an external program
        run_exec(cli, exec_cmd, &computed_style, &features)
    } else if cli.should_read_stdin() {
        // Read from stdin
        run_stdin(cli, &computed_style, &features)
    } else {
        // Process files
        run_files(cli, &computed_style, &features)
    }
}

/// Load configuration with optional overrides.
fn load_config(cli: &Cli) -> io::Result<Config> {
    let mut config = Config::load().unwrap_or_default();

    // Apply config override if provided
    if let Some(ref config_arg) = cli.config {
        if Path::new(config_arg).exists() {
            // It's a file path
            match Config::load_from(Path::new(config_arg)) {
                Ok(override_config) => {
                    config.merge(&override_config);
                    debug!("Merged config from file: {}", config_arg);
                }
                Err(e) => {
                    error!("Failed to load config file {}: {}", config_arg, e);
                }
            }
        } else {
            // Try parsing as inline TOML
            match toml::from_str::<Config>(config_arg) {
                Ok(override_config) => {
                    config.merge(&override_config);
                    debug!("Merged inline config");
                }
                Err(e) => {
                    error!("Failed to parse config: {}", e);
                }
            }
        }
    }

    // Apply HSV base if provided
    if let Some((h, s, v)) = cli.parse_base() {
        debug!("Setting HSV base: {}, {}, {}", h, s, v);
        // TODO: Apply HSV base to computed style
    }

    Ok(config)
}

/// Create render features from CLI options.
fn create_features(cli: &Cli) -> RenderFeatures {
    let (fixed_width, width_wrap) = if cli.width > 0 {
        (Some(cli.width as usize), false)
    } else {
        (None, true)
    };

    RenderFeatures {
        pretty_pad: !cli.no_pretty_pad,
        pretty_broken: cli.pretty_broken,
        clipboard: cli.clipboard,
        savebrace: cli.savebrace,
        fixed_width,
        width_wrap,
        ..Default::default()
    }
}

/// Process input from stdin.
fn run_stdin(cli: &Cli, style: &ComputedStyle, features: &RenderFeatures) -> io::Result<()> {
    info!("Reading from stdin");

    let stdin = io::stdin();
    let width = cli.effective_width();
    let render_style = RenderStyle::from_computed(style);
    let theme = cli.theme.clone();
    let no_highlight = cli.no_highlight;

    // Use a buffer for output
    let mut output = Vec::new();
    let mut parser = MarkdownParser::new();
    let mut plugin_manager = PluginManager::with_builtins();
    let parse_state = streamdown_core::state::ParseState::new();

    // Read stdin line by line for streaming
    for line in stdin.lock().lines() {
        let line = line?;
        trace!("Input line: {}", line);

        // Check plugins first
        if let Some(plugin_output) = plugin_manager.process_line(&line, &parse_state, style) {
            for output_line in plugin_output {
                writeln!(output, "{}", output_line)?;
            }
            // Flush output
            io::stdout().write_all(&output)?;
            io::stdout().flush()?;
            output.clear();
            continue;
        }

        // Parse and render
        {
            let mut renderer = Renderer::with_style(&mut output, width, render_style.clone());
            renderer.set_features(features.clone());
            if !no_highlight {
                renderer.set_theme(&theme);
            }
            emit_line(&line, &mut parser, &mut renderer, cli)?;
        }

        // Flush output
        io::stdout().write_all(&output)?;
        io::stdout().flush()?;
        output.clear();
    }

    // Flush any remaining plugin content
    let plugin_output = plugin_manager.flush();
    for line in plugin_output {
        writeln!(io::stdout(), "{}", line)?;
    }

    io::stdout().flush()?;
    Ok(())
}

/// Process input files.
fn run_files(cli: &Cli, style: &ComputedStyle, features: &RenderFeatures) -> io::Result<()> {
    let width = cli.effective_width();
    let render_style = RenderStyle::from_computed(style);
    let theme = cli.theme.clone();
    let no_highlight = cli.no_highlight;

    for path in &cli.files {
        info!("Processing file: {}", path.display());

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut output = Vec::new();
        let mut parser = MarkdownParser::new();
        let mut plugin_manager = PluginManager::with_builtins();
        let parse_state = streamdown_core::state::ParseState::new();

        for line in reader.lines() {
            let line = line?;

            // Check plugins first
            if let Some(plugin_output) = plugin_manager.process_line(&line, &parse_state, style) {
                for output_line in plugin_output {
                    writeln!(output, "{}", output_line)?;
                }
                continue;
            }

            // Parse and render
            {
                let mut renderer = Renderer::with_style(&mut output, width, render_style.clone());
                renderer.set_features(features.clone());
                if !no_highlight {
                    renderer.set_theme(&theme);
                }
                emit_line(&line, &mut parser, &mut renderer, cli)?;
            }
        }

        // Flush remaining plugin content
        let plugin_output = plugin_manager.flush();
        for line in plugin_output {
            writeln!(output, "{}", line)?;
        }

        // Write all output
        io::stdout().write_all(&output)?;
    }

    io::stdout().flush()?;
    Ok(())
}

/// Run with an exec'd subprocess using PTY.
fn run_exec(
    cli: &Cli,
    exec_cmd: &str,
    style: &ComputedStyle,
    features: &RenderFeatures,
) -> io::Result<()> {
    use pty::{PollResult, PtySession};
    use regex::Regex;
    use std::time::Duration;

    // Check if PTY is supported
    if !pty::is_supported() {
        return Err(pty::unsupported_error());
    }

    info!("Executing with PTY: {}", exec_cmd);

    let width = cli.effective_width();
    let render_style = RenderStyle::from_computed(style);
    let theme = cli.theme.clone();
    let no_highlight = cli.no_highlight;

    // Compile prompt regex
    let prompt_regex =
        Regex::new(&cli.prompt).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    // Spawn PTY session
    let mut session = PtySession::spawn(exec_cmd)?;
    info!("PTY session started");

    let mut output = Vec::new();
    let mut parser = MarkdownParser::new();
    let mut plugin_manager = PluginManager::with_builtins();
    let parse_state = streamdown_core::state::ParseState::new();

    // Line buffer for accumulating output
    let mut line_buffer = String::new();
    let timeout = Duration::from_millis(100);

    // Main loop
    while session.is_alive() {
        match session.poll(timeout) {
            PollResult::Stdin | PollResult::Both => {
                // Keyboard input from user
                if let Some(byte) = session.read_stdin_byte()? {
                    // Forward to subprocess
                    session.write_master_byte(byte)?;

                    // Handle newline
                    if byte == b'\n' || byte == b'\r' {
                        // Flush current line
                        if !line_buffer.is_empty() {
                            line_buffer.clear();
                        }
                        println!();
                        session.reset_keyboard_count();
                    }
                }

                // Also check for subprocess output if Both
                if matches!(
                    session.poll(Duration::ZERO),
                    PollResult::Master | PollResult::Both
                ) {
                    process_master_output(
                        &mut session,
                        &mut line_buffer,
                        &mut output,
                        &mut parser,
                        &mut plugin_manager,
                        &parse_state,
                        style,
                        &prompt_regex,
                        width,
                        &render_style,
                        &theme,
                        no_highlight,
                        features,
                        cli,
                    )?;
                }
            }
            PollResult::Master => {
                // Output from subprocess
                process_master_output(
                    &mut session,
                    &mut line_buffer,
                    &mut output,
                    &mut parser,
                    &mut plugin_manager,
                    &parse_state,
                    style,
                    &prompt_regex,
                    width,
                    &render_style,
                    &theme,
                    no_highlight,
                    features,
                    cli,
                )?;
            }
            PollResult::Timeout => {
                // Check if there's a partial line that might be a prompt
                if !line_buffer.is_empty() {
                    let visible = streamdown_ansi::utils::visible(&line_buffer);
                    if prompt_regex.is_match(&visible) {
                        // This looks like a prompt, emit it directly
                        print!("{}", line_buffer);
                        io::stdout().flush()?;
                        line_buffer.clear();
                    }
                }
            }
            PollResult::Error => {
                break;
            }
        }
    }

    // Flush remaining content
    if !line_buffer.is_empty() {
        println!("{}", line_buffer);
    }

    let plugin_output = plugin_manager.flush();
    for line in plugin_output {
        writeln!(io::stdout(), "{}", line)?;
    }

    io::stdout().flush()?;

    // Wait for child
    let exit_code = session.wait()?;
    debug!("Child exited with: {}", exit_code);

    Ok(())
}

/// Process output from the master side of the PTY.
#[allow(clippy::too_many_arguments)]
fn process_master_output(
    session: &mut pty::PtySession,
    line_buffer: &mut String,
    output: &mut Vec<u8>,
    parser: &mut MarkdownParser,
    plugin_manager: &mut PluginManager,
    parse_state: &streamdown_core::state::ParseState,
    style: &ComputedStyle,
    prompt_regex: &regex::Regex,
    width: usize,
    render_style: &RenderStyle,
    theme: &str,
    no_highlight: bool,
    features: &RenderFeatures,
    cli: &Cli,
) -> io::Result<()> {
    let mut buf = [0u8; 1024];

    loop {
        let n = session.read_master(&mut buf)?;
        if n == 0 {
            break;
        }

        // If user is typing, echo the output directly
        if session.keyboard_count() > 0 {
            io::stdout().write_all(&buf[..n])?;
            io::stdout().flush()?;
            continue;
        }

        // Process bytes into lines
        for &byte in &buf[..n] {
            if byte == b'\n' {
                // Complete line
                let line = std::mem::take(line_buffer);
                trace!("PTY line: {}", line);

                // Check for prompt
                let visible = streamdown_ansi::utils::visible(&line);
                if prompt_regex.is_match(&visible) {
                    // Pass through prompt directly
                    println!("{}", line);
                    io::stdout().flush()?;
                    continue;
                }

                // Check plugins
                if let Some(plugin_output) = plugin_manager.process_line(&line, parse_state, style)
                {
                    for output_line in plugin_output {
                        writeln!(output, "{}", output_line)?;
                    }
                    io::stdout().write_all(output)?;
                    io::stdout().flush()?;
                    output.clear();
                    continue;
                }

                // Parse and render
                {
                    let mut renderer =
                        Renderer::with_style(&mut *output, width, render_style.clone());
                    renderer.set_features(features.clone());
                    if !no_highlight {
                        renderer.set_theme(theme);
                    }
                    emit_line(&line, parser, &mut renderer, cli)?;
                }

                // Flush output
                io::stdout().write_all(output)?;
                io::stdout().flush()?;
                output.clear();
            } else if byte == b'\r' {
                // Ignore carriage returns
            } else {
                line_buffer.push(byte as char);
            }
        }
    }

    Ok(())
}

/// Emit a single line through the parser and renderer.
fn emit_line<W: Write>(
    line: &str,
    parser: &mut MarkdownParser,
    renderer: &mut Renderer<W>,
    cli: &Cli,
) -> io::Result<()> {
    // Parse the line and get events
    let events = parser.parse_line(line);

    // Process all events
    for event in events {
        trace!("Parse event: {:?}", event);

        // Handle code scraping if enabled
        if let Some(ref scrape_dir) = cli.scrape {
            scrape_code(&event, scrape_dir)?;
        }

        // Render the event
        renderer.render_event(&event)?;
    }

    Ok(())
}

/// Scrape code blocks to a directory.
fn scrape_code(event: &ParseEvent, scrape_dir: &Path) -> io::Result<()> {
    static CODE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    match event {
        ParseEvent::CodeBlockStart { language, .. } => {
            // Create scrape directory if needed
            std::fs::create_dir_all(scrape_dir)?;

            let counter = CODE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let raw_ext = language.as_deref().unwrap_or("txt");
            // Sanitize extension to prevent path traversal attacks
            let ext = streamdown_ansi::sanitize::sanitize_extension(raw_ext);
            let ext = if ext.is_empty() {
                "txt".to_string()
            } else {
                ext
            };
            // Use modulo to prevent filename overflow with large counters
            let filename = format!("code_{:08}.{}", counter % 100_000_000, ext);
            let path = scrape_dir.join(&filename);

            debug!("Scraping code to: {}", path.display());

            // Create empty file (will be appended to)
            File::create(&path)?;
        }
        ParseEvent::CodeBlockLine(line) => {
            // Append to the most recent code file
            let counter = CODE_COUNTER.load(std::sync::atomic::Ordering::SeqCst);
            if counter > 0 {
                // Find the file
                for entry in std::fs::read_dir(scrape_dir)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    if name
                        .to_string_lossy()
                        .starts_with(&format!("code_{:08}", (counter - 1) % 100_000_000))
                    {
                        let mut file = std::fs::OpenOptions::new()
                            .append(true)
                            .open(entry.path())?;
                        writeln!(file, "{}", line)?;
                        break;
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_features() {
        let cli = Cli::parse_from(["sd"]);
        let features = create_features(&cli);

        assert!(features.pretty_pad);
        assert!(!features.pretty_broken);
        assert!(!features.clipboard);
    }

    #[test]
    fn test_create_features_with_options() {
        let cli = Cli::parse_from([
            "sd",
            "--no-pretty-pad",
            "--pretty-broken",
            "--clipboard",
            "--savebrace",
        ]);
        let features = create_features(&cli);

        assert!(!features.pretty_pad);
        assert!(features.pretty_broken);
        assert!(features.clipboard);
        assert!(features.savebrace);
    }

    #[test]
    fn test_create_features_with_width() {
        let cli = Cli::parse_from(["sd", "-w", "100"]);
        let features = create_features(&cli);

        assert_eq!(features.fixed_width, Some(100));
        assert!(!features.width_wrap);
    }
}
