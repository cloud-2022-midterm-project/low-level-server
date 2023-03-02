pub(crate) fn get_content_length(lines: &mut std::str::Lines) -> usize {
    lines
        .find_map(|line| {
            if line.starts_with("content-length: ") {
                line.trim_start_matches("content-length: ").parse().ok()
            } else if line.starts_with("Content-Length: ") {
                line.trim_start_matches("Content-Length: ").parse().ok()
            } else {
                None
            }
        })
        .expect("Content-Length header not found")
}
