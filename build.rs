use glib_build_tools::compile_resources;

fn main() {
    compile_resources(
        &["data"],
        "data/resources.gresource.xml",
        "resources.gresource",
    );
}
