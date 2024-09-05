use std::{
    convert::Infallible,
    fmt::Display,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use homedir::my_home;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// A simple ssh connection management tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Jump {
    #[command(subcommand)]
    opt: Opt,
}

#[derive(Debug, Subcommand)]
enum Opt {
    /// Initialize the jump database
    Initialize,
    /// Add a server to current store
    Add(Server),
    /// Remove a server in current store
    Rm { server_name: String },
    /// List all servers in current store
    Ls,
    /// Connecting to server
    Conn { server_name: String },
}

#[derive(Debug, Args, Serialize, Deserialize)]
struct Server {
    server_name: String,
    username: String,
    // #[arg(value_parser  = parse_ip)]
    server_address: String,
    #[arg(default_value = "22")]
    port: u32,
    #[command(subcommand)]
    method: ConnectMethods,
}

#[derive(Debug, Subcommand, Serialize, Deserialize)]
enum ConnectMethods {
    SSHKey(SSHKey),
    Password(Password),
}

#[derive(Debug, Args, Serialize, Deserialize)]
struct SSHKey {
    #[arg(value_parser = parse_ssh_path, default_value = "~/.ssh/id_rsa")]
    path: PathBuf,
}

fn parse_ssh_path(str: &str) -> Result<PathBuf, Infallible> {
    str.try_into()
}

#[derive(Debug, Parser, Serialize, Deserialize)]
struct Password {
    password: String,
}

impl Display for ConnectMethods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectMethods::SSHKey(key) => {
                write!(f, "ssh:{}", key.path.to_str().unwrap())
            }
            ConnectMethods::Password(p) => write!(f, "pass:{}", p.password),
        }
    }
}

impl From<String> for ConnectMethods {
    fn from(method: String) -> Self {
        let v = method.split(":").collect::<Vec<_>>();
        match v[0] {
            "ssh" => ConnectMethods::SSHKey(SSHKey {
                path: PathBuf::from_str(v[1]).unwrap(),
            }),
            _ => ConnectMethods::Password(Password {
                password: v[1].to_owned(),
            }),
        }
    }
}

fn main() -> Result<()> {
    let args = Jump::parse();
    let mut home = my_home()?.unwrap();
    home.push(".jump/servers.db");
    let conn = Connection::open(home)?;

    match args.opt {
        Opt::Initialize => initialize(conn),
        Opt::Add(server) => add_server(conn, server),
        Opt::Rm { server_name } => remove_server(conn, server_name),
        Opt::Ls => list_servers(conn),
        Opt::Conn { server_name } => connect_to_server(conn, server_name),
    }
}

fn initialize(conn: Connection) -> Result<()> {
    conn.execute(
        "create table if not exists jump_servers (
             id integer primary key,
             server_name text not null unique,
             username text not null,
             server_address text not null,
             port integer not null,
             method text not null)",
        [],
    )?;
    Ok(())
}

fn add_server(conn: Connection, server: Server) -> Result<()> {
    conn.execute(
        "INSERT INTO jump_servers (server_name, username, server_address, port, method) values (?1, ?2, ?3, ?4, ?5)",
        [server.server_name, server.username, server.server_address, server.port.to_string(), server.method.to_string()],
    )?;
    Ok(())
}

fn remove_server(conn: Connection, server_name: String) -> Result<()> {
    conn.execute(
        "DELETE FROM jump_servers WHERE server_name = ?1",
        [server_name],
    )?;
    Ok(())
}

fn list_servers(conn: Connection) -> Result<()> {
    let mut stmt = conn
        .prepare("SELECT server_name, username, server_address, port, method FROM jump_servers")?;
    let servers = stmt.query_map([], |row| {
        let method_string: String = row.get(4)?;
        Ok(Server {
            server_name: row.get(0)?,
            username: row.get(1)?,
            server_address: row.get(2)?,
            port: row.get(3)?,
            method: ConnectMethods::from(method_string),
        })
    })?;
    for server in servers {
        let server = server?;
        println!(
            "{} username: {} address: {}",
            server.server_name, server.username, server.server_address
        );
    }
    Ok(())
}

fn connect_to_server(conn: Connection, server_name: String) -> Result<()> {
    let mut stmt = conn
    .prepare("SELECT server_name, username, server_address, port, method FROM jump_servers where server_name = ?1")?;
    let server = stmt.query_row([server_name], |row| {
        let method_string: String = row.get(4)?;
        Ok(Server {
            server_name: row.get(0)?,
            username: row.get(1)?,
            server_address: row.get(2)?,
            port: row.get(3)?,
            method: ConnectMethods::from(method_string),
        })
    })?;
    println!("connecting to server...");
    match server.method {
        ConnectMethods::Password(Password { password }) => {
            Command::new("sshpass")
                .args(vec![
                    "-p",
                    &password,
                    "ssh",
                    "-p",
                    &server.port.to_string(),
                    &format!("{}@{}", server.username, server.server_address),
                ])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?;
        }
        ConnectMethods::SSHKey(SSHKey { path }) => {
            Command::new("ssh")
                .args(vec![
                    "-i",
                    path.to_str().ok_or(anyhow!("Invalid ssh key path"))?,
                    "-p",
                    &server.port.to_string(),
                    &format!("{}@{}", server.username, server.server_address),
                ])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?;
        }
    }
    println!("server disconnected");
    Ok(())
}
