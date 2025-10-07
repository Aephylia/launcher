extern crate embed_resource;

fn main() {
    // Replace "app-manifest.rc" with the .rc filename you add below if you choose a different name
    embed_resource::compile("app-manifest.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
