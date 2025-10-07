use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

mod process;
mod injection;

use process::{suspend_process, kill_process_by_name};
use injection::inject_dll;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    path: String,

    #[arg(long)]
    email: String,

    #[arg(long)]
    pass: String,

    #[arg(long, default_value = "false")]
    no_restart: bool,

    #[arg(long)]
    redirect_dll: Option<String>,

    #[arg(long)]
    gameserver_dll: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("[+] Path: {}", args.path);
    println!("[+] Email: {}", args.email);
    println!();

    let redirect_dll = args
        .redirect_dll
        .clone()
        .unwrap_or_else(|| "redirect.dll".to_string());
    let gameserver_dll = args
        .gameserver_dll
        .clone()
        .unwrap_or_else(|| "gameserver.dll".to_string());

    if !Path::new(&redirect_dll).exists() {
        eprintln!("[!] Error: redirect.dll not found at: {}", redirect_dll);
        return Ok(());
    }
    if !Path::new(&gameserver_dll).exists() {
        eprintln!("[!] Error: gameserver.dll not found at: {}", gameserver_dll);
        return Ok(());
    }

    loop {
        match launch_game_instance(&args, &redirect_dll, &gameserver_dll).await {
            Ok(_) => {
                if args.no_restart {
                    println!("[+] Game finished. Exiting...");
                    break;
                } else {
                    println!("[+] Game finished. Restarting in 2 seconds...");
                    thread::sleep(Duration::from_secs(2));
                }
            }
            Err(e) => {
                eprintln!("[!] Error launching game: {}", e);
                if args.no_restart {
                    break;
                }
                thread::sleep(Duration::from_secs(5));
            }
        }
    }

    Ok(())
}

async fn launch_game_instance(
    args: &Args,
    redirect_dll: &str,
    gameserver_dll: &str,
) -> Result<()> {
    let base_path = PathBuf::from(&args.path);

    let launcher_path = base_path
        .join("FortniteGame")
        .join("Binaries")
        .join("Win64")
        .join("FortniteLauncher.exe");

    if !launcher_path.exists() {
        anyhow::bail!("FortniteLauncher.exe not found at: {:?}", launcher_path);
    }

    println!("[+] Starting FortniteLauncher.exe...");
    let launcher_child = Command::new(&launcher_path)
        .spawn()
        .context("Failed to start FortniteLauncher")?;

    let launcher_pid = launcher_child.id();
    thread::sleep(Duration::from_millis(500));
    suspend_process(launcher_pid)?;
    println!("[+] FortniteLauncher suspended (PID: {})", launcher_pid);

    let eac_path = base_path
        .join("FortniteGame")
        .join("Binaries")
        .join("Win64")
        .join("FortniteClient-Win64-Shipping_EAC.exe");

    if !eac_path.exists() {
        anyhow::bail!("FortniteClient-Win64-Shipping_EAC.exe not found at: {:?}", eac_path);
    }

    println!("[+] Starting EAC process...");
    let eac_child = Command::new(&eac_path)
        .spawn()
        .context("Failed to start EAC")?;

    let eac_pid = eac_child.id();
    thread::sleep(Duration::from_millis(500));
    suspend_process(eac_pid)?;
    println!("[+] EAC process suspended (PID: {})", eac_pid);

    let fortnite_path = base_path
        .join("FortniteGame")
        .join("Binaries")
        .join("Win64")
        .join("FortniteClient-Win64-Shipping.exe");

    if !fortnite_path.exists() {
        anyhow::bail!("FortniteClient-Win64-Shipping.exe not found at: {:?}", fortnite_path);
    }

    let launch_args = format!(
        r#"-epicapp=Fortnite -epicenv=Prod -epiclocale=en-us -epicportal -skippatchcheck -nobe -fromfl=eac -log -fltoken=3db3ba5dcbd2e16703f3978d -caldera=eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9.eyJhY2NvdW50X2lkIjoiYmU5ZGE1YzJmYmVhNDQwN2IyZjQwZWJhYWQ4NTlhZDQiLCJnZW5lcmF0ZWQiOjE2Mzg3MTcyNzgsImNhbGRlcmFHdWlkIjoiMzgxMGI4NjMtMmE2NS00NDU3LTliNTgtNGRhYjNiNDgyYTg2IiwiYWNQcm92aWRlciI6IkVhc3lBbnRpQ2hlYXQiLCJub3RlcyI6IiIsImZhbGxiYWNrIjpmYWxzZX0.VAWQB67RTxhiWOxx7DBjnzDnXyyEnX7OljJm-j2d88G_WgwQ9wrE6lwMEHZHjBd1ISJdUO1UVUqkfLdU5nofBQ -AUTH_LOGIN={} -AUTH_PASSWORD={} -AUTH_TYPE=epic"#,
        args.email, args.pass
    );

    println!("[+] Starting Fortnite client...");
    let mut fortnite_child = TokioCommand::new(&fortnite_path)
        .args(launch_args.split_whitespace())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start Fortnite")?;

    let fortnite_pid = fortnite_child.id().context("Failed to get Fortnite PID")?;
    println!("[+] Fortnite client started (PID: {})", fortnite_pid);

    thread::sleep(Duration::from_millis(1000));
    match inject_dll(fortnite_pid, redirect_dll) {
        Ok(_) => println!("[+] redirect.dll injected successfully"),
        Err(e) => eprintln!("[!] Failed to inject redirect.dll: {}", e),
    }

    let stdout = fortnite_child.stdout.take().context("Failed to get stdout")?;
    let stderr = fortnite_child.stderr.take().context("Failed to get stderr")?;

    let gameserver_dll_clone = gameserver_dll.to_string();
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut injected = false;

        while let Ok(Some(line)) = lines.next_line().await {
            if line.contains("Region ") && !injected {
                injected = true;
                println!("[+] Region detected, waiting 10 seconds before gameserver injection...");
                tokio::time::sleep(Duration::from_secs(10)).await;

                match inject_dll(fortnite_pid, &gameserver_dll_clone) {
                    Ok(_) => println!("[+] gameserver.dll injected successfully"),
                    Err(e) => eprintln!("[!] Failed to inject gameserver.dll: {}", e),
                }
            }

            let login_errors = [
                "port 3551 failed: Connection refused",
                "Unable to login to Fortnite servers",
                "HTTP 400 response from",
                "Network failure when attempting to check platform restrictions",
                "UOnlineAccountCommon::ForceLogout",
            ];

            if login_errors.iter().any(|err| line.contains(err)) {
                eprintln!("[!] Login error detected: {}", line);
                eprintln!("[!] Killing process and restarting...");
                let _ = kill_process_by_name("FortniteClient-Win64-Shipping");
                break;
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[ERROR] {}", line);
        }
    });

    let status = fortnite_child.wait().await?;
    println!("[+] Fortnite exited with status: {}", status);

    stdout_task.abort();
    stderr_task.abort();

    println!("[+] Cleaning up processes...");
    let processes_to_kill = [
        "EpicGamesLauncher",
        "EpicWebHelper",
        "CrashReportClient",
        "FortniteLauncher",
        "FortniteClient-Win64-Shipping",
        "EasyAntiCheat_EOS",
        "FortniteClient-Win64-Shipping_EAC",
    ];

    for process_name in &processes_to_kill {
        let _ = kill_process_by_name(process_name);
    }

    Ok(())
}
