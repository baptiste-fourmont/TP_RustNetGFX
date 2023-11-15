extern crate piston_window;

use piston_window::*;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
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
        let mut bytes = Vec::with_capacity(size);
        bytes.push(opcode_byte[0]);
        bytes.extend(buffer);

        Opcode::from_bytes(&bytes)
    }
}

fn main() -> Result<(), std::io::Error> {
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;
    let mut user_id = 1;
    let mut positions: std::collections::HashMap<u32, [f64; 2]> = std::collections::HashMap::new();

    match Opcode::receive_opcode(&mut stream) {
        Ok(opcode) => match opcode {
            Opcode::Welcome { id } => {
                println!("Welcome message received with ID {}", id);
                user_id = id;
            }
            Opcode::Move { x, y, id } => {
                if id != user_id {
                    positions.insert(id, [x.into(), y.into()]);
                }
            }
            Opcode::Disconnect { id } => {
                positions.remove(&id);
            }
        },
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to receive opcode",
            ))
        }
    }

    let mut position = [0.0, 0.0];
    let window_title = format!("Player {}", user_id);
    let mut window: PistonWindow = WindowSettings::new(window_title, [512; 2])
        .build()
        .unwrap();

    let (tx, rx) = std::sync::mpsc::channel();

    let mut stream_clone = stream.try_clone().expect("Failed to clone stream");
    std::thread::spawn(move || {
        loop {
            match Opcode::receive_opcode(&mut stream_clone) {
                Ok(opcode) => {
                    if let Opcode::Move { x, y, id } = opcode {
                        println!("Received move message from client {}: ({}, {})", id, x, y);
                        let move_message = (id, [x.into(), y.into()], 0);
                        tx.send(move_message).unwrap();
                    }
                    if let Opcode::Disconnect { id } = opcode {
                        let disconnect_message = (id, [0.into(), 0.into()], 1);
                        tx.send(disconnect_message).unwrap();
                    }
                }
                Err(e) => {
                    println!("Error receiving opcode: {}", e);
                    break; // Vous voudrez peut-être sortir de la boucle si une erreur se produit
                }
            }
        }
    });

    while let Some(e) = window.next() {
        // Recevoir des positions mises à jour ou traiter l'absence de données
        while let Ok((id, pos, state)) = rx.try_recv() {
            if state == 1 {
                positions.remove(&id);
            } else if id != user_id {
                println!("Updating position for client {}: {:?}", id, pos); // Débogage ajouté ici
                positions.insert(id, pos);
            } else {
                // Update own position
                println!("Updating position for client {}: {:?}", id, pos); // Débogage ajouté ici
                position = pos;
            }
        }

        if let Some(Button::Keyboard(key)) = e.press_args() {
            let mut moved = false;
            match key {
                Key::Left => {
                    position[0] -= 5.0;
                    moved = true;
                }
                Key::Right => {
                    position[0] += 5.0;
                    moved = true;
                }
                Key::Down => {
                    position[1] += 5.0;
                    moved = true;
                }
                Key::Up => {
                    position[1] -= 5.0;
                    moved = true;
                }
                _ => (),
            }

            if moved {
                let move_message = Opcode::Move {
                    x: position[0] as i32,
                    y: position[1] as i32,
                    id: user_id as u32,
                };
                println!("Sending move message: {:?}", move_message);
                stream.write_all(&move_message.asbytes())?;
            }
        }

        // Gérer les positions actuelles même en l'absence de nouvelles données reçues
        window.draw_2d(&e, |c, g, _| {
            clear([0.5, 0.5, 0.5, 1.0], g);
            rectangle(
                color::BLUE,
                [position[0], position[1], 100.0, 100.0],
                c.transform,
                g,
            );
            for (_id, position) in &positions {
                rectangle(
                    color::RED,
                    [position[0], position[1], 100.0, 100.0],
                    c.transform,
                    g,
                );
            }
        });
    }
    let disconnect_message = Opcode::Disconnect { id: user_id };
    println!("Sending move message: {:?}", disconnect_message);
    stream.write_all(&disconnect_message.asbytes())?;

    Ok(())
}
