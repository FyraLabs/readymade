#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:ident $msg:literal $body:block) => {{
        let s = tracing::info_span!(concat!("stage-", stringify!($s)));

        crate::playbook::PROGRESS_SENDER.with_borrow(|tx| {
            tx.as_ref()
                .expect("couldn't get progress sender")
                .send(crate::playbook::PlaybookProgress::Stage($msg.to_owned()))
                .expect("couldn't send progress");
        });

        {
            let _guard = s.enter();
            tracing::debug!("Entering stage");
            $body
        }
    }};
}
