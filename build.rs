use glib_build_tools::compile_resources;

fn main() {
    compile_resources(&["data"], "data/icons.gresource.xml", "icons.gresource");
}
