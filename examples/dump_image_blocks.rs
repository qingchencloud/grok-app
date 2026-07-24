//! One-shot: dump ACP image prompt blocks from a PNG path (verification helper).
use grok_app::attachments::{build_prompt_blocks, from_path};
use std::env;
use std::path::PathBuf;

fn main() {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: dump_image_blocks <png> [out.json]");
    let out = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("image_blocks.json"));
    let img = from_path(&path).expect("load image");
    let blocks = build_prompt_blocks("describe this fixture", &[img]);
    let json = serde_json::to_string_pretty(&blocks).expect("json");
    std::fs::write(&out, &json).expect("write");
    println!("wrote {} ({} blocks)", out.display(), blocks.len());
}
