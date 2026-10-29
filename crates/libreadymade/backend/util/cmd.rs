/// Run a command, checking its exit status.
///
/// ```ignore
/// cmd!("umount" [["-R"], [path]] => |status| bail!("umount failed: {:?}", status.code()));
/// ```
///
/// Each bracketed group is passed to [`std::process::Command::args`]. If the command cannot be
/// spawned, the enclosing function returns an error; if it exits unsuccessfully, `$err` is
/// evaluated with the [`std::process::ExitStatus`] bound to `$cmd`.
#[macro_export]
macro_rules! cmd {
    ($process:literal $([$($args:expr),+$(,)?])? => |$cmd:tt| $err:expr) => {
        let status = $crate::prelude::WrapErr::wrap_err(
            ::std::process::Command::new($process)
                $(
                    $(.args($args))+
                )?
                .status(),
            concat!("fail to execute ", $process),
        )?;
        if !status.success() {
            #[allow(clippy::let_underscore_untyped)]
            let $cmd = status;
            $err;
        }
    };
}
