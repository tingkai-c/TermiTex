//! Nonblocking renderer interface. Only the worker thread waits for typesetting.
use crate::{config::Renderer, detect::Formula};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub key: String,
    pub formula: Formula,
    #[serde(default)]
    pub compatibility: bool,
    pub cell_width: u16,
    pub cell_height: u16,
}
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub key: String,
    #[serde(default)]
    pub png: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub columns: u16,
}

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
};

pub trait MathRenderer {
    /// Returns false when the bounded queue is full or the worker has exited.
    fn submit(&mut self, request: Request) -> bool;
    fn poll(&mut self) -> Option<Response>;
    fn stop(&mut self);
}

pub struct ChannelRenderer {
    pub tx: SyncSender<Request>,
    pub rx: Receiver<Response>,
}
impl MathRenderer for ChannelRenderer {
    fn submit(&mut self, request: Request) -> bool {
        self.tx.try_send(request).is_ok()
    }
    fn poll(&mut self) -> Option<Response> {
        self.rx.try_recv().ok()
    }
    fn stop(&mut self) {}
}

pub struct WorkerRenderer {
    channel: ChannelRenderer,
    child: Child,
}
impl WorkerRenderer {
    pub fn spawn(renderer: Renderer) -> io::Result<Self> {
        let mut command = match renderer {
            Renderer::Ratex => {
                let mut cmd = Command::new(std::env::current_exe()?);
                cmd.arg("--internal-ratex-worker");
                cmd
            }
            Renderer::Mathjax => {
                let path =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worker/render.mjs");
                if !path.is_file() {
                    return Err(io::Error::other(
                        "MathJax requires a source checkout with npm dependencies; packaged builds provide native RaTeX",
                    ));
                }
                let mut cmd = Command::new("node");
                cmd.arg(path);
                cmd
            }
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            for req in requests {
                let result = (|| -> io::Result<Response> {
                    serde_json::to_writer(&mut input, &req)?;
                    input.write_all(b"\n")?;
                    input.flush()?;
                    let mut line = String::new();
                    reader.by_ref().take(17_000_000).read_line(&mut line)?;
                    serde_json::from_str(&line).map_err(io::Error::other)
                })();
                let r = result.unwrap_or_else(|e| Response {
                    key: req.key,
                    png: String::new(),
                    columns: 0,
                    error: Some(e.to_string()),
                });
                if responses.send(r).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            channel: ChannelRenderer { tx, rx },
            child,
        })
    }
}
impl MathRenderer for WorkerRenderer {
    fn submit(&mut self, request: Request) -> bool {
        self.channel.submit(request)
    }
    fn poll(&mut self) -> Option<Response> {
        self.channel.poll()
    }
    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for WorkerRenderer {
    fn drop(&mut self) {
        self.stop();
    }
}
