use anyhow::Result;
use ratatui::{prelude::*, widgets::*};
use crossterm::{event, execute, terminal};
use crossterm::event::{Event, KeyCode, KeyModifiers};

#[derive(Clone, Copy, Debug)]
enum StreamKind { Stdout, Stderr }

#[derive(Debug)]
struct LineMsg { idx: usize, kind: StreamKind, text: String }

struct App {
    logs: Vec<Vec<String>>, // one buffer per command
    selected: usize,
}

impl App {
    fn new(n: usize) -> Self { Self { logs: vec![Vec::new(); n], selected: 0 } }
    fn push(&mut self, msg: LineMsg) {
        let buf = &mut self.logs[msg.idx];
        if buf.len() > 10_000 { buf.drain(..5_000); }
        let prefix = match msg.kind { StreamKind::Stdout => "", StreamKind::Stderr => "[err] " };
        buf.push(format!("{}{}", prefix, msg.text));
    }
}

async fn spawn_reader(
    mut child: tokio::process::Child,
    idx: usize,
    tx: tokio::sync::mpsc::Sender<LineMsg>,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<i32> {
    use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut out = AsyncBufReader::new(stdout).lines();
    let mut err = AsyncBufReader::new(stderr).lines();

    let tx_out = tx.clone();
    let t_out = tokio::spawn(async move {
        while let Ok(Some(line)) = out.next_line().await {
            let _ = tx_out.send(LineMsg { idx, kind: StreamKind::Stdout, text: line }).await;
        }
    });

    let tx_err = tx.clone();
    let t_err = tokio::spawn(async move {
        while let Ok(Some(line)) = err.next_line().await {
            let _ = tx_err.send(LineMsg { idx, kind: StreamKind::Stderr, text: line }).await;
        }
    });

    // Wait for process to exit or cancellation request
    let status = tokio::select! {
        res = child.wait() => { res? }
        _ = cancel_rx.changed() => {
            // Attempt to kill the child and wait for it to exit
            let _ = child.kill().await;
            child.wait().await?
        }
    };

    let _ = t_out.await; let _ = t_err.await;
    Ok(status.code().unwrap_or(-1))
}

pub async fn run(commands: Vec<String>) -> Result<i32> {
    use std::io;
    use std::time::Duration;

    // terminal setup
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // channels
    let (tx, mut rx) = tokio::sync::mpsc::channel::<LineMsg>(1024);
    // cancellation signal for spawned processes
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel::<bool>(false);

    // spawn processes
    let mut join_handles = Vec::new();
    for (idx, cmd) in commands.iter().enumerate() {
        #[cfg(windows)]
        let child = tokio::process::Command::new("cmd").arg("/C").arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        #[cfg(not(windows))]
        let child = tokio::process::Command::new("sh").arg("-c").arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let txc = tx.clone();
        let crx = cancel_rx.clone();
        join_handles.push(tokio::spawn(spawn_reader(child, idx, txc, crx)));
    }
    drop(tx);

    let mut app = App::new(commands.len());

    // main loop
    loop {
        // draw
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ]).split(f.area());

            let titles: Vec<Line> = commands.iter().enumerate().map(|(i, c)| {
                let title = format!("{}: {}", i+1, c);
                Line::from(Span::styled(title, Style::default().fg(if i==app.selected { Color::Yellow } else { Color::White })))
            }).collect();
            let tabs = Tabs::new(titles).select(app.selected);
            f.render_widget(tabs, chunks[0]);

            let items: Vec<ListItem> = app.logs[app.selected]
                .iter().rev().take(1000).rev()
                .map(|l| ListItem::new(l.as_str()))
                .collect();
            let list = List::new(items).block(Block::default().title("Output").borders(Borders::ALL));
            f.render_widget(list, chunks[1]);

            let help = Paragraph::new("q=quit  ←/→=pane  Tab=next  Shift-Tab=prev");
            f.render_widget(help, chunks[2]);
        })?;

        // drain new lines with a short timeout
        let mut drained = 0;
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
            app.push(msg);
            drained += 1;
            if drained > 10_000 { break; }
        }

        // input
        if event::poll(Duration::from_millis(10))?
            && let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Left => { app.selected = app.selected.saturating_sub(1); }
                KeyCode::Right => { app.selected = (app.selected + 1).min(commands.len()-1); }
                KeyCode::Tab => { if !commands.is_empty() { app.selected = (app.selected + 1) % commands.len(); } }
                KeyCode::BackTab => { if !commands.is_empty() { app.selected = if app.selected == 0 { commands.len()-1 } else { app.selected - 1 }; } }
                _ => {}
            }
        }
    }

    // teardown
    terminal::disable_raw_mode()?;
    execute!(std::io::stdout(), terminal::LeaveAlternateScreen, event::DisableMouseCapture)?;

    // request cancellation/kill of all running processes
    let _ = cancel_tx.send(true);

    // gather exit codes
    let mut worst = 0;
    for h in join_handles { if let Ok(Ok(code)) = h.await && code != 0 { worst = code; } }
    Ok(if worst < 0 { 1 } else { worst })
}
