use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self},
    thread,
};

pub enum Opcode {
    Welcome { id: u32 },
    Move { x: i32, y: i32, id: u32 },
    Disconnect { id: u32 },
}

impl Opcode {
    fn asbytes(&self) -> Vec<u8> {
        match self {
            Opcode::Welcome { id } => {
                let mut bytes = Vec::new();
                bytes.push(0x00);
                bytes.extend(id.to_le_bytes());
                bytes
            }
            Opcode::Move { x, y, id } => {
                let mut bytes = Vec::new();
                bytes.push(0x01);
                bytes.extend(x.to_le_bytes());
                bytes.extend(y.to_le_bytes());
                bytes.extend(id.to_le_bytes());
                bytes
            }
            Opcode::Disconnect { id } => {
                let mut bytes = Vec::new();
                bytes.push(0x02);
                bytes.extend(id.to_le_bytes());
                bytes
            }
        }
    }
    fn from_bytes(bytes: &[u8]) -> Result<Opcode, std::io::Error> {
        match bytes[0] {
            0x00 => {
                if bytes.len() != 5 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Invalid number of bytes for Welcome opcode",
                    ));
                }

                Ok(Opcode::Welcome {
                    id: u32::from_le_bytes(bytes[1..5].try_into().unwrap()),
                })
            }
            0x01 => {
                if bytes.len() != 13 {
                    dbg!(bytes);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Invalid number of bytes for Move opcode",
                    ));
                }

                Ok(Opcode::Move {
                    x: i32::from_le_bytes(bytes[1..5].try_into().unwrap()),
                    y: i32::from_le_bytes(bytes[5..9].try_into().unwrap()),
                    id: u32::from_le_bytes(bytes[9..13].try_into().unwrap()),
                })
            }
            0x02 => {
                if bytes.len() != 5 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Invalid number of bytes for Disconnect opcode",
                    ));
                }

                Ok(Opcode::Disconnect {
                    id: u32::from_le_bytes(bytes[1..5].try_into().unwrap()),
                })
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Invalid Opcode",
            )),
        }
    }
    fn receive_opcode(stream: &mut TcpStream) -> Result<Opcode, std::io::Error> {
        let mut opcode_byte = [0u8; 1];
        stream.read_exact(&mut opcode_byte)?;

        let size = match opcode_byte[0] {
            0x00 => 5,  // Welcome has an ID (4 bytes) + opcode byte (1 byte)
            0x01 => 13, // Move has two coordinates (8 bytes) + ID (4 bytes) + opcode byte (1 byte)
            0x02 => 5,  // Disconnect has an ID (4 bytes) + opcode byte (1 byte)
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Invalid Opcode",
                ));
            }
        };

        let mut buffer = vec![0u8; size - 1];
        stream.read_exact(&mut buffer)?;

        // Combine opcode_byte and buffer into one Vec<u8>
        let mut bytes: Vec<u8> = Vec::with_capacity(size);
        bytes.push(opcode_byte[0]);
        bytes.extend(buffer);

        Opcode::from_bytes(&bytes)
    }
}

fn broadcast_loop(rx: mpsc::Receiver<Vec<u8>>, client_streams: mpsc::Receiver<TcpStream>) {
    let mut clients: HashMap<u32, TcpStream> = HashMap::new();
    let mut client_id: u32 = 1;

    loop {
        // Écouter le canal pour de nouveaux messages ou de nouveaux clients
        for message in rx.try_iter() {
            for stream in clients.values_mut() {
                let _ = stream.write_all(&message);
            }
        }

        // Ajouter de nouveaux clients
        for stream in client_streams.try_iter() {
            clients.insert(client_id, stream);
            client_id += 1;
        }
    }
}

fn handle_client(mut stream: TcpStream, broadcast_tx: mpsc::Sender<Vec<u8>>, client_id: u32) {
    // This function checks if the movement is in increments of 5 pixels
    // 'x' and 'y' are the new desired positions of a game entity
    fn is_move_valid(x: i32, y: i32) -> bool {
        // Check if both the x and y positions are multiples of 5
        x % 5 == 0 && y % 5 == 0
    }

    fn is_delta_valid(old: Option<i32>, new: i32) -> bool {
        if let Some(old) = old {
            (new - old).abs() % 5 == 0
        } else {
            true // First Move
        }
    }

    let mut old_x: Option<i32> = None;
    let mut old_y: Option<i32> = None;

    loop {
        // This function "receive_opcode" wait for an opcode and send it into bytes to the ("Manager of Broadcast")
        match Opcode::receive_opcode(&mut stream) {
            Ok(opcode) => {
                // Vérification des opcodes
                let is_valid = match opcode {
                    Opcode::Move { x, y, id } => {
                        // Verify if the move is valid and the id matches the client's id
                        id == client_id
                            && is_move_valid(x, y)
                            && is_delta_valid(old_x, x)
                            && is_delta_valid(old_y, y)
                    }
                    Opcode::Welcome { id } => id == client_id,
                    Opcode::Disconnect { id } => id == client_id,
                };

                if is_valid {
                    // If the opcode is valid, convert it to bytes and send it
                    let opcode_bytes = opcode.asbytes();
                    broadcast_tx
                        .send(opcode_bytes)
                        .expect("Failed to send to broadcaster");

                    if let Opcode::Move { x, y, id: _ } = opcode {
                        old_x = Some(x);
                        old_y = Some(y);
                    }
                } else {
                    // Handle invalid opcode or incorrect id
                    println!(
                        "Received invalid opcode or incorrect id from client {}",
                        client_id
                    );

                    stream
                        .shutdown(std::net::Shutdown::Both)
                        .expect("Failed to close the stream");
                    println!("Close the Connection with Tcheater");
                    break;
                }
            }
            Err(e) => {
                println!("Error receiving opcode from client: {}", e);
                let disconnect_opcode = if let Ok(opcode) = Opcode::receive_opcode(&mut stream) {
                    opcode
                } else {
                    // If we cannot receive the opcode, we construct a disconnect message
                    Opcode::Disconnect { id: client_id }
                };
                let disconnect_message = disconnect_opcode.asbytes();
                let _ = broadcast_tx.send(disconnect_message);
                stream
                    .shutdown(std::net::Shutdown::Both)
                    .expect("Failed to close the stream");
                println!("Close the Connection with Tcheater");
                break;
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to create listener");
    let (broadcast_tx, broadcast_rx) = mpsc::channel::<Vec<u8>>();
    let (client_tx, client_rx) = mpsc::channel::<TcpStream>();

    std::thread::spawn(move || {
        broadcast_loop(broadcast_rx, client_rx);
    });

    let mut user_id = 1;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let client_id = user_id;
                user_id += 1;
                let stream_clone = stream.try_clone().expect("Failed to clone stream");

                let welcome_message = Opcode::Welcome { id: client_id };
                let welcome_bytes: Vec<u8> = welcome_message.asbytes();
                let _ = stream.write_all(&welcome_bytes);

                client_tx
                    .send(stream_clone)
                    .expect("Failed to send stream to broadcaster");

                let broadcast_tx_clone = broadcast_tx.clone();
                // Spawn a new thread to handle the client's incoming messages
                thread::spawn(move || handle_client(stream, broadcast_tx_clone, client_id));
            }
            Err(e) => {
                println!("Error reading opcode: {}", e);
            }
        }
    }
}
