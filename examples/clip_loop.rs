fn main() {
    use grok_app::attachments;
    for i in 0..5 {
        let hint = attachments::clipboard_has_image_hint();
        let r = attachments::from_clipboard_ex();
        println!(
            "#{i} hint={hint} => {:?}",
            r.as_ref().map(|o| o
                .as_ref()
                .map(|img| (img.width, img.height, img.name.as_str())))
        );
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
