use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use portable_pty::{Child, CommandBuilder, MasterPty, native_pty_system};

pub use portable_pty::{ExitStatus, PtySize};

fn to_io(err: impl std::fmt::Display) -> io::Error {
    io::Error::other(err.to_string())
}

/// 起動するコマンドの指定。テストから任意のコマンドを注入できるようにしている。
#[derive(Debug, Clone)]
pub struct PtyCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
    pub size: PtySize,
}

impl PtyCommand {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            size: PtySize::default(),
        }
    }

    pub fn default_shell() -> Self {
        let program = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
        Self::new(program)
    }

    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    pub fn cwd<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.cwd = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(mut self, key: K, value: V) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    pub fn size(mut self, size: PtySize) -> Self {
        self.size = size;
        self
    }

    fn to_builder(&self) -> CommandBuilder {
        let mut builder = CommandBuilder::new(&self.program);
        builder.args(&self.args);
        if let Some(cwd) = &self.cwd {
            builder.cwd(cwd);
        }
        for (key, value) in &self.env {
            builder.env(key, value);
        }
        builder
    }
}

pub struct PtyEngine {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader: Option<Box<dyn Read + Send>>,
    writer: Option<Box<dyn Write + Send>>,
}

impl PtyEngine {
    pub fn spawn(command: &PtyCommand) -> io::Result<Self> {
        let pair = native_pty_system().openpty(command.size).map_err(to_io)?;
        let child = pair
            .slave
            .spawn_command(command.to_builder())
            .map_err(to_io)?;

        // 親側のslave fdを閉じないと子が終了してもmaster側がEOFにならない。
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().map_err(to_io)?;
        let writer = pair.master.take_writer().map_err(to_io)?;

        Ok(Self {
            master: pair.master,
            child,
            reader: Some(reader),
            writer: Some(writer),
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::other("pty writer already taken"))?;
        writer.write_all(bytes)?;
        writer.flush()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::other("pty reader already taken"))?;
        reader.read(buf)
    }

    pub fn take_reader(&mut self) -> io::Result<Box<dyn Read + Send>> {
        self.reader
            .take()
            .ok_or_else(|| io::Error::other("pty reader already taken"))
    }

    pub fn take_writer(&mut self) -> io::Result<Box<dyn Write + Send>> {
        self.writer
            .take()
            .ok_or_else(|| io::Error::other("pty writer already taken"))
    }

    pub fn resize(&self, size: PtySize) -> io::Result<()> {
        self.master.resize(size).map_err(to_io)
    }

    pub fn size(&self) -> io::Result<PtySize> {
        self.master.get_size().map_err(to_io)
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }
}
