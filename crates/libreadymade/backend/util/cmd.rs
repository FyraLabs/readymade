use std::{
    io::{BufRead, BufReader},
    process::{Child, Output},
};

pub fn pipe_cmd<const N: usize, F: FnOnce() + Send>(
    msg: &str,
    mut cmd: Child,
    f: [F; N],
) -> (std::io::Result<Output>, String) {
    let logs: std::sync::Arc<parking_lot::Mutex<String>> = std::sync::Arc::default();
    let stdout = cmd.stdout.take().expect("can't take stdout");
    let stderr = cmd.stderr.take().expect("can't take stderr");
    println!("┌─ BEGIN: {msg}");
    let cmd = std::thread::scope(|s| {
        s.spawn(|| {
            let reader = BufReader::new(stdout);
            (reader.lines().map(|line| line.unwrap())).for_each(|line| {
                *logs.lock() += &line;
                *logs.lock() += "\n";
                println!(" │ {line}");
            });
        });
        s.spawn(|| {
            let reader = BufReader::new(stderr);
            (reader.lines().map(|line| line.unwrap())).for_each(|line| {
                *logs.lock() += &line;
                *logs.lock() += "\n";
                eprintln!("!│ {line}");
            });
        });
        for f in f {
            s.spawn(f);
        }
        cmd.wait_with_output()
    });
    let logs = std::sync::Arc::into_inner(logs).unwrap().into_inner();
    println!("└─ END OF {msg}");
    (cmd, logs)
}

// NOTE: this doesn't work well with [I; N] which requires all iterators to have the same type…
/*
pub fn cmd<const N: usize, I, S>(process: &str, argss: [I; N]) -> ExitStatus
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new(process);
    for args in argss {
        cmd.args(args);
    }
    cmd.status()
        .unwrap_or_else(|_| panic!("fail to execute {process}"))
}*/

#[macro_export]
macro_rules! cmd {
    ($process:literal $([$($args:expr),+$(,)?])? => |$cmd:tt| $err:expr) => {
        let cmd = Command::new($process)
            $(
                $(.args($args))+
            )?
            .status()
            .context(const_format::concatcp!("fail to execute ", $process))?;
        if !cmd.success() {
            #[allow(clippy::let_underscore_untyped)]
            let $cmd = cmd;
            $err;
        }
    };
}
