use slint_build::CompilerConfiguration;

fn main() {
    let cfg = CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", cfg).expect("cannot compile ui");
}
