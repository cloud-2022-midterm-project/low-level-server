use std::str::FromStr;

#[derive(Debug)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Default for Method {
    fn default() -> Self {
        Self::Get
    }
}

impl FromStr for Method {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            _ => Err("Invalid method"),
        }
    }
}
