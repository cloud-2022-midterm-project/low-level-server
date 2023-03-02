use tokio::{
    io::{self, AsyncReadExt},
    net::TcpStream,
};

pub(crate) fn get_content_length(lines: &mut std::str::Lines) -> Option<usize> {
    lines.find_map(|line| {
        if line.starts_with("content-length: ") {
            line.trim_start_matches("content-length: ").parse().ok()
        } else if line.starts_with("Content-Length: ") {
            line.trim_start_matches("Content-Length: ").parse().ok()
        } else {
            None
        }
    })
}

pub(crate) async fn read_stream_request(
    stream: &mut TcpStream,
) -> io::Result<(String, Option<usize>)> {
    let mut request_buf: Vec<u8> = Vec::new();
    let mut request_len = 0;

    let mut checked_content_length = false;
    let mut content_length: Option<usize> = None;

    const BUF_SIZE: usize = 4096;

    loop {
        let mut tmp_buf = [0; BUF_SIZE];

        let bytes_read = match stream.read(&mut tmp_buf).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error: {}", e);
                return Err(e);
            }
        };
        request_len += bytes_read;
        request_buf.extend_from_slice(&tmp_buf[..bytes_read]);

        // check if we have content-length
        if !checked_content_length {
            checked_content_length = true;

            if let Some(len) =
                get_content_length(&mut std::str::from_utf8(&request_buf).unwrap().lines())
            {
                content_length = Some(len);
                request_buf.reserve(len + bytes_read);
            }
        }

        // find if we have read the whole stream
        if bytes_read < BUF_SIZE {
            match content_length {
                // we can actually read faster than the incoming stream, so we need to wait
                // check if we have read the expected whole body
                Some(content_length) => {
                    if request_len > content_length {
                        break;
                    }
                }
                // we don't have expected content-length, so we can break
                None => {
                    break;
                }
            }
        }
    }

    Ok((
        std::str::from_utf8(&request_buf).unwrap().to_string(),
        content_length,
    ))
}
