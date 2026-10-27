#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
---
// std-only Cargo script; game crates continue to use Rust stable.
use std::{
    env,
    error::Error,
    io::{BufRead, BufReader},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
unsafe extern "system" fn on_console_event(event: u32) -> i32 {
    if event <= 1 {
        INTERRUPTED.store(true, Ordering::SeqCst);
        1
    } else {
        0
    }
}

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW for console helpers
    }
    command
}

struct Server(Child);
impl Server {
    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        drop(self.0.stdin.take());
        let status = self.0.wait()?;
        if !status.success() {
            return Err(
                format!("Server exited with {status}; inspect the save error above").into(),
            );
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        // Never terminate the authority forcibly: let it finish its final save.
        drop(self.0.stdin.take());
        let _ = self.0.wait();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    if env::args().any(|arg| arg == "--help") {
        println!(
            "Run just dev from the repository to start server and client together.\nDirect invocation also accepts Godot game arguments, e.g. --name Player."
        );
        return Ok(());
    }
    let repo = env::current_dir()?;
    if !repo.join("scripts/godot.ps1").is_file() {
        return Err("Run from the repository root (or use just dev)".into());
    }
    let mut address: SocketAddr = env::var("CDDA_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9876".into())
        .parse()?;
    if address.port() == 0 {
        return Err("just dev requires a fixed nonzero CDDA_BIND port".into());
    }
    if address.ip().is_unspecified() {
        address.set_ip(match address.ip() {
            IpAddr::V4(_) => Ipv4Addr::LOCALHOST.into(),
            IpAddr::V6(_) => Ipv6Addr::LOCALHOST.into(),
        });
    }
    let certificate =
        repo.join(env::var_os("CDDA_CERT_PATH").unwrap_or_else(|| "save/server.cert.der".into()));
    // Do all compilation before opening the world or launching a GUI.
    if !Command::new("just")
        .arg("build")
        .env_remove("RUSTUP_TOOLCHAIN")
        .status()?
        .success()
    {
        return Err("Build failed".into());
    }
    // Cargo honors CARGO_TARGET_DIR and workspace configuration; query its target path.
    let metadata = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()?;
    if !metadata.status.success() {
        return Err("Cargo metadata failed".into());
    }
    // Reuse PowerShell's JSON decoder rather than adding script dependencies.
    let mut decoder = hidden(Command::new("powershell").args([
        "-NoProfile",
        "-Command",
        "($input | Out-String | ConvertFrom-Json).target_directory",
    ]))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
    use std::io::Write;
    decoder.stdin.take().unwrap().write_all(&metadata.stdout)?;
    let decoded = decoder.wait_with_output()?;
    if !decoded.status.success() {
        return Err("Cannot read Cargo target directory".into());
    }
    let target = PathBuf::from(String::from_utf8(decoded.stdout)?.trim());

    #[cfg(windows)]
    {
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn SetConsoleCtrlHandler(
                handler: unsafe extern "system" fn(u32) -> i32,
                add: i32,
            ) -> i32;
        }
        if unsafe { SetConsoleCtrlHandler(on_console_event, 1) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    let mut server = Server(
        hidden(&mut Command::new(target.join("debug/server.exe")))
            .env("CDDA_SHUTDOWN_ON_STDIN_CLOSE", "1")
            .env("RUST_LOG", "server=info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let output = server.0.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if line.contains("Server listening") {
                let _ = ready_tx.send(());
            }
            println!("{line}");
        }
    });
    let start = Instant::now();
    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            return server.stop();
        }
        if let Some(status) = server.0.try_wait()? {
            return Err(format!("Server startup failed: {status}").into());
        }
        if ready_rx.try_recv().is_ok() {
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            return Err("Server startup timed out".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    println!("Connecting client to {address}. Closing the client saves and stops this server.");
    let mut client = hidden(
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "scripts/godot.ps1",
                "run",
                "--connect",
                "--server",
                &address.to_string(),
                "--cert",
            ])
            .arg(certificate)
            .args(env::args_os().skip(1)),
    )
    .env_remove("RUSTUP_TOOLCHAIN")
    .spawn()?;
    let result = loop {
        if let Some(status) = client.try_wait()? {
            break status.success();
        }
        if INTERRUPTED.load(Ordering::SeqCst) || server.0.try_wait()?.is_some() {
            // End the client helper and its GUI; only the server owns saved state.
            let _ = hidden(Command::new("taskkill").args([
                "/PID",
                &client.id().to_string(),
                "/T",
                "/F",
            ]))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
            let _ = client.wait();
            break false;
        }
        thread::sleep(Duration::from_millis(100));
    };
    server.stop()?;
    if !result {
        return Err("Client interrupted or exited with an error".into());
    }
    Ok(())
}
