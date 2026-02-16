#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:ident $msg:literal $body:block) => {{
        let s = tracing::info_span!(concat!("stage-", stringify!($s)));

        // if let Some(m) = $crate::backend::install::IPC_CHANNEL.get() {
        //     let sender = m.lock();
        //     // Then we are in a non-interactive install, which means we export IPC
        //     // to stdout
        //     let install_status =
        //         $crate::backend::install::InstallationMessage::Status($msg.to_string());
        //     sender.send(install_status).expect("cannot send");
        // }

        {
            let _guard = s.enter();
            tracing::debug!("Entering stage");
            $body
        }
    }};
}
