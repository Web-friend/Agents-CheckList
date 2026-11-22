use std::net::TcpStream;
use std::str;
use std::io::{self, Write, Read, BufRead, BufReader, Write };

fn main() -> io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;

    loop {
        let mut input = String::new();
        let mut buffer : Vec<u8> = Vec::new();
        io::stdin().read_line(&mut input).expect("Failed to read input");
        stream.write_all(input.as_bytes()).expect("Failed to send message");
        let mut reader = BufReader::new(&stream);
        reader.read_until(b'\n', &mut buffer).expect("Failed to read response");
        let response = str::from_utf8(&buffer).expect("Failed to convert response to
    string");
        print!("Server response: {}", response);
        buffer.clear();

    }
}