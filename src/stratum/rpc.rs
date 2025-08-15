pub mod request;
pub mod response;

use request::Request;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{self, BufReader, BufWriter, Write, BufRead},
    net::TcpStream,
};

pub fn send<S: Serialize>(
    writer: &mut BufWriter<TcpStream>,
    request: &Request<S>,
) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, request)?;
    writeln!(writer)?;
    writer.flush()?;
    Ok(())
}

pub fn recv<D: DeserializeOwned>(reader: &mut BufReader<TcpStream>) -> serde_json::Result<D> {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(serde_json::Error::io)?;
    if line.is_empty() {
        return Err(serde_json::Error::io(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF while reading line")));
    }
    serde_json::from_str(&line)
}
