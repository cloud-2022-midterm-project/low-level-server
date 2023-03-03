pub mod method;

use std::error::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpStream,
};

use self::method::Method;

impl Default for Method {
    fn default() -> Self {
        Self::Get
    }
}

#[derive(Default, Debug)]
pub struct Request {
    method: Method,
    uri: String,
    body: Option<String>,
}

impl Request {
    /// Read data from the stream and create a new HTTP `Request`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the data from the stream is invalid HTTP request.
    pub async fn from_stream(stream: &mut TcpStream) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut buf_reader = BufReader::new(stream);

        // read status line
        let mut status_line = String::with_capacity(512);
        buf_reader.read_line(&mut status_line).await?;
        let status_line = status_line.trim_end();

        let mut request = Self::default();

        // extract method and uri
        let mut status_line_iter = status_line.split_whitespace();
        let method = status_line_iter.next().unwrap_or("");
        request.set_method(method)?;
        let uri = status_line_iter.next().unwrap_or("").to_string();
        request.set_uri(uri);

        // read through header section and find content-length if any
        let mut content_length = None;
        let mut header_line = String::with_capacity(512);
        loop {
            buf_reader.read_line(&mut header_line).await?;
            let trimmed = header_line.trim_end();

            match trimmed {
                // end of header section
                "" => break,
                // find content-length
                l if content_length.is_none() => {
                    let mut header_line_iter = l.splitn(2, ": ");
                    let header_name = header_line_iter.next().unwrap_or("");
                    if header_name.eq_ignore_ascii_case("content-length") {
                        let header_value = header_line_iter.next().unwrap_or("");
                        content_length = Some(header_value.parse()?);
                    }
                }
                _ => (),
            }

            // stream's read_line() will append a newline to the end of the line
            // we need an empty string to read the next line
            header_line.clear();
        }

        // read body if any
        if let Some(len) = content_length {
            let mut body = vec![0; len];
            buf_reader.read_exact(&mut body).await?;
            let body = String::from_utf8_lossy(&body).to_string();
            request.set_body(Some(body));
        }

        Ok(request)
    }

    pub fn uri(&self) -> &str {
        self.uri.as_ref()
    }

    pub fn set_uri(&mut self, uri: String) {
        self.uri = uri;
    }

    pub fn body(&self) -> Option<&String> {
        self.body.as_ref()
    }

    pub fn set_body(&mut self, body: Option<String>) {
        self.body = body;
    }

    pub fn method(&self) -> &method::Method {
        &self.method
    }

    /// Sets the method of this [`Request`]. If the method is invalid, this function will return an
    /// error.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn set_method(&mut self, method: &str) -> Result<(), &'static str> {
        self.method = method.parse()?;
        Ok(())
    }
}
