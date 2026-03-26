//! Skylet Chat Test Interface
//!
//! A CLI interface to test chatbot and workflow functionality.
//!
//! Usage:
//!   skylet-chat                    - Interactive chat mode
//!   skylet-chat "Research AI"      - Single message mode
//!   skylet-chat --status <id>      - Check workflow status
//!   skylet-chat --list             - List workflows

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::Client;
use std::io::{self, BufRead, Write};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "skylet-chat")]
#[command(about = "Test interface for Skylet chatbot and workflows")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    #[arg(short, long, default_value = "http://127.0.0.1:8080")]
    server: String,
    
    #[arg(short, long, default_value = "test-user")]
    user: String,
    
    #[arg(long, default_value = "cli")]
    platform: String,
    
    message: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    List {
        #[arg(short, long, default_value = "http://127.0.0.1:8080")]
        server: String,
    },
    Status {
        #[arg(short, long)]
        execution_id: String,
        #[arg(short, long, default_value = "http://127.0.0.1:8080")]
        server: String,
    },
    Watch {
        #[arg(short, long)]
        execution_id: String,
        #[arg(short, long, default_value = "http://127.0.0.1:8080")]
        server: String,
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },
}

fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();
    
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match cli.command {
            Some(Commands::List { server }) => {
                list_workflows(&server).await?;
            }
            Some(Commands::Status { execution_id, server }) => {
                check_status(&server, &execution_id).await?;
            }
            Some(Commands::Watch { execution_id, server, interval }) => {
                watch_execution(&server, &execution_id, interval).await?;
            }
            None => {
                if let Some(msg) = cli.message {
                    send_message(&cli.server, &cli.user, &cli.platform, &msg).await?;
                } else {
                    interactive_mode(&cli.server, &cli.user, &cli.platform).await?;
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    })?;
    
    Ok(())
}

async fn send_message(server: &str, user: &str, platform: &str, message: &str) -> Result<()> {
    let client = Client::new();
    
    let payload = serde_json::json!({
        "action": "chat",
        "user_id": user,
        "platform": platform,
        "message": message,
    });
    
    println!("\n> {}", message);
    println!("\n{:-<60}", "");
    
    let response = client
        .post(format!("{}/rpc/chatbot", server))
        .json(&payload)
        .send()
        .await?;
    
    let status = response.status();
    let body = response.text().await?;
    
    if !status.is_success() {
        println!("Error: HTTP {} - {}", status, body);
        return Ok(());
    }
    
    let json: serde_json::Value = serde_json::from_str(&body)?;
    
    if let Some(result) = json.get("result").or(json.get("response")) {
        println!("{}", pretty_print_json(result));
    } else if let Some(error) = json.get("error") {
        println!("Error: {}", error);
    } else {
        println!("{}", pretty_print_json(&json));
    }
    
    Ok(())
}

async fn interactive_mode(server: &str, user: &str, platform: &str) -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║           Skylet Chat Test Interface                       ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  Commands:                                                 ║");
    println!("║    /list        - List available workflows                 ║");
    println!("║    /status <id> - Check workflow status                    ║");
    println!("║    /watch <id>  - Watch workflow progress                  ║");
    println!("║    /clear       - Clear screen                             ║");
    println!("║    /quit        - Exit                                     ║");
    println!("║    /help        - Show this help                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    print!("> ");
    stdout.flush()?;
    
    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        
        if trimmed.is_empty() {
            print!("> ");
            stdout.flush()?;
            continue;
        }
        
        match trimmed {
            "/quit" | "/exit" => {
                println!("Goodbye!");
                break;
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                print!("> ");
                stdout.flush()?;
                continue;
            }
            "/help" => {
                println!("Commands: /list, /status <id>, /watch <id>, /clear, /quit, /help");
                print!("> ");
                stdout.flush()?;
                continue;
            }
            "/list" => {
                list_workflows(server).await?;
                print!("> ");
                stdout.flush()?;
                continue;
            }
            s if s.starts_with("/status ") => {
                let id = s.strip_prefix("/status ").unwrap();
                check_status(server, id).await?;
                print!("> ");
                stdout.flush()?;
                continue;
            }
            s if s.starts_with("/watch ") => {
                let id = s.strip_prefix("/watch ").unwrap();
                watch_execution(server, id, 3).await?;
                print!("> ");
                stdout.flush()?;
                continue;
            }
            _ => {
                if let Err(e) = send_message(server, user, platform, trimmed).await {
                    println!("Error: {}", e);
                }
            }
        }
        
        print!("> ");
        stdout.flush()?;
    }
    
    Ok(())
}

async fn list_workflows(server: &str) -> Result<()> {
    let client = Client::new();
    
    let payload = serde_json::json!({
        "action": "list",
    });
    
    let response = client
        .post(format!("{}/rpc/workflow", server))
        .json(&payload)
        .send()
        .await?;
    
    let body = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    
    println!("\n📋 Available Workflows:");
    println!("{}", "─".repeat(60));
    
    if let Some(workflows) = json.get("workflows").and_then(|w| w.as_array()) {
        for wf in workflows {
            let id = wf.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let name = wf.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let desc = wf.get("description").and_then(|v| v.as_str()).unwrap_or("");
            println!("  • {} - {}", style(id, "cyan"), name);
            if !desc.is_empty() {
                println!("    {}", style(desc, "dimmed"));
            }
        }
    } else {
        println!("  No workflows found or error: {}", pretty_print_json(&json));
    }
    println!();
    
    Ok(())
}

async fn check_status(server: &str, execution_id: &str) -> Result<()> {
    let client = Client::new();
    
    let payload = serde_json::json!({
        "action": "status",
        "execution_id": execution_id,
    });
    
    let response = client
        .post(format!("{}/rpc/workflow", server))
        .json(&payload)
        .send()
        .await?;
    
    let body = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    
    println!("\n📊 Workflow Status: {}", execution_id);
    println!("{}", "─".repeat(60));
    
    if let Some(exec) = json.get("execution") {
        let status = exec.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
        let progress = exec.get("progress_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let step = exec.get("current_step").and_then(|v| v.as_str()).unwrap_or("none");
        
        let status_style = match status {
            "Completed" => style(status, "green"),
            "Failed" => style(status, "red"),
            "Running" => style(status, "yellow"),
            _ => style(status, "white"),
        };
        
        println!("  Status:   {}", status_style);
        println!("  Progress: {:.0}%", progress);
        println!("  Step:     {}", step);
        
        if let Some(result) = exec.get("result") {
            println!("\n  Result:");
            println!("{}", indent(&pretty_print_json(result), "    "));
        }
    } else if let Some(error) = json.get("error") {
        println!("  Error: {}", error);
    } else {
        println!("  {}", pretty_print_json(&json));
    }
    println!();
    
    Ok(())
}

async fn watch_execution(server: &str, execution_id: &str, interval_secs: u64) -> Result<()> {
    println!("\n👀 Watching workflow: {}", execution_id);
    println!("    (Press Ctrl+C to stop)\n");
    
    let client = Client::new();
    let mut last_progress = -1.0;
    
    loop {
        let payload = serde_json::json!({
            "action": "status",
            "execution_id": execution_id,
        });
        
        let response = client
            .post(format!("{}/rpc/workflow", server))
            .json(&payload)
            .send()
            .await?;
        
        let body = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&body)?;
        
        if let Some(exec) = json.get("execution") {
            let status = exec.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
            let progress = exec.get("progress_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let step = exec.get("current_step").and_then(|v| v.as_str()).unwrap_or("");
            
            if progress != last_progress {
                let bar = progress_bar(progress as f32, 40);
                println!("\r  [{}] {:.0}% - {}", bar, progress, step);
                last_progress = progress;
            }
            
            if status == "Completed" || status == "Failed" || status == "Cancelled" {
                println!("\n  Final status: {}", status);
                if let Some(result) = exec.get("result") {
                    if let Some(report) = result.get("report") {
                        println!("\n📄 Report:");
                        println!("{}", indent(report.as_str().unwrap_or(""), "  "));
                    }
                }
                break;
            }
        } else if let Some(error) = json.get("error") {
            println!("\n  Error: {}", error);
            break;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }
    
    Ok(())
}

fn progress_bar(progress: f32, width: usize) -> String {
    let filled = (progress / 100.0 * width as f32) as usize;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn pretty_print_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        _ => value.to_string(),
    }
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn style(text: &str, style_name: &str) -> String {
    let codes = match style_name {
        "cyan" => "\x1B[36m",
        "green" => "\x1B[32m",
        "yellow" => "\x1B[33m",
        "red" => "\x1B[31m",
        "dimmed" => "\x1B[2m",
        "bold" => "\x1B[1m",
        _ => "",
    };
    if codes.is_empty() {
        text.to_string()
    } else {
        format!("{}{}\x1B[0m", codes, text)
    }
}
