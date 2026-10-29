#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:ident $msg:literal $body:block) => {{
        let s = tracing::info_span!(concat!("stage-", stringify!($s)));

        $crate::playbook::report_progress($crate::playbook::PlaybookProgress::Stage(
            $msg.to_owned(),
        ));

        {
            let _guard = s.enter();
            tracing::debug!("Entering stage");
            $body
        }
    }};
}
