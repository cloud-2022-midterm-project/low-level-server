use std::fmt;

pub(crate) struct Response<'a> {
    pub(crate) status_line: &'a str,
    pub(crate) headers: Vec<&'a str>,
    pub(crate) content: Option<&'a str>,
}

impl<'a> Response<'a> {
    pub(crate) fn new() -> Self {
        Self {
            status_line: "HTTP/1.1 200 OK",
            headers: Vec::new(),
            content: None,
        }
    }

    pub(crate) fn status_line(mut self, status_line: &'a str) -> Self {
        self.status_line = status_line;
        self
    }

    pub(crate) fn set_status_line(&mut self, status_line: &'a str) {
        self.status_line = status_line;
    }

    pub(crate) fn append_header(mut self, header: &'a str) -> Self {
        self.headers.push(header);
        self
    }

    pub(crate) fn body(mut self, content: &'a str) -> Self {
        self.content = Some(content);
        self
    }
}

impl<'a> fmt::Display for Response<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut response = String::new();
        response.push_str(self.status_line);
        response.push_str("\r\n");
        for header in self.headers.iter() {
            response.push_str(header);
            response.push_str("\r\n");
        }
        response.push_str("\r\n");
        if let Some(content) = self.content {
            response.push_str(content);
        }
        write!(f, "{}", response)
    }
}
