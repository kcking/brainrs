use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    // Bind the UDP socket to the desired port
    let socket = UdpSocket::bind("0.0.0.0:8002")?;
    socket.set_broadcast(true).unwrap();
    println!("Listening on UDP port 8002...");

    let mut buf = [0; 1024]; // Buffer to store incoming data

    loop {
        // Receive data from the socket
        let (amt, src) = socket.recv_from(&mut buf)?;

        // Convert the data to hexadecimal format
        let hex_data: String = buf[..amt]
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<Vec<String>>()
            .join(" ");

        // Print the received packet in hexadecimal format and its source address
        println!("Received {} bytes from {}: {}", amt, src, hex_data);
    }
}
