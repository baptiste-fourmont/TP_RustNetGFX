# Networked Graphics Application in Rust

## Overview

Ce projet est une application de réseau graphique en Rust, combinant les capacités graphiques de `piston_window` avec une communication réseau robuste via des sockets TCP. Il permet la connexion de multiples clients à un serveur, gérant les interactions telles que les mouvements des joueurs et les déconnexions.

## Features

### Communication Réseau TCP

- Utilisation de `TcpStream` et `TcpListener` pour établir et gérer des connexions réseau.
- Les données sont échangées sous forme d'opcodes personnalisés (`Welcome`, `Move`, `Disconnect`) pour une communication efficace et structurée.
- Méthode `receive_opcode` pour lire et interpréter les opcodes reçus.

### Graphiques avec Piston Window

- Utilisation de `piston_window` pour créer une fenêtre graphique et dessiner des entités.
- Gestion de la position et du rendu des entités en fonction des messages réseau.

### Gestion des OpCodes

`Opcode::asbytes` et `Opcode::from_bytes` pour la sérialisation et la désérialisation des opcodes.

Il existe des Opcodes distincts pour différentes actions (`Welcome`, `Move`, `Disconnect`)

<mark>Welcome</mark>

```bash
0        1                            5
+--------+----------------------------+
| Opcode |            ID              |
+--------+----------------------------+
|  0x00  | u32(4 bytes,little-endian) |
+--------+----------------------------+
```

<mark>Move</mark>

```bash
+--------+--------------------------+--------------------------+----------------------------+
| Opcode |            X             |            Y             |             ID             |
+--------+--------------------------+--------------------------+----------------------------+
|  0x01  | i32 (4 bytes, little-endian) | i32 (4 bytes, little-endian) | u32 (4 bytes, little-endian) |
+--------+--------------------------+--------------------------+----------------------------+
```

<mark>Disconnect</mark>

```bash
0        1                            5
+--------+----------------------------+
| Opcode |            ID              |
+--------+----------------------------+
|  0x02  | u32(4 bytes,little-endian) |
+--------+----------------------------+ 
```

### Gestion des Utilisateurs

- Assignation d'ID uniques aux clients lors de la connexion.
- Suivi et mise à jour des positions des joueurs en temps réel.
- Gestion des déconnexions pour maintenir l'état correct du jeu.

### Architecture de communication

Modèle client-serveur : Utilisation de TCP pour la communication directe client-serveur.

Passage de messages : Utilisation de mpsc::channel pour la gestion asynchrone des messages entre les threads sans utiliser d'état partagé.

```bash
+--------------------------------------------------------------+
|                      TCP Listener                            |
| +-------------------+  listens on 127.0.0.1:8080             |
| |                   |  accepts new connections               |
| |                   |  assigns unique IDs                    |
| |                   +-----+                                  |
| +-------------------+     |                                  |
+---------------------------|----------------------------------+
                            v
      +---------------------+------+         +------------------+
      | New Client Connection     |         |                  |
      | +---------------------+   |         |                  |
      | | Send Welcome Opcode  |   |         |                  |
      | +---------------------+   |         |                  |
      +---------------------------|------+  |                  |
                                  |      |  |                  |
                                  v      |  |                  |
     +----------------+    +------+      |  |     Broadcast    |
     | Clone Stream   +--->+ client_tx   +------+   Thread     |
     +----------------+    +------+      |  |                  |
                                  |      |  |                  |
                                  |      |  |                  |
                                  v      |  |                  |
                                +--------+--+                  |
                                | broadcast_tx    +---------+  |
                                +---------------->+         |  |
                                  |                | Loop   |  |
                                  |                |        |  |
+-----------------+               |                +---+----+  |
|  Client Handler |               |                    |       |
|     Thread      +<--------------+                    |       |
| (One per client)|                                    |       |
+-----------------+                                    |       |
                                                        |       |
                    +-----------------+                |       |
                    | Opcode Handling |<---------------+       |
                    | Validation      |                        |
                    | Move/Disconnect |                        |
                    +-----------------+                        |
                                                               |
                    +-----------------+                        |
                    | Broadcasting    |                        |
                    | to all clients  |<-----------------------+
                    | if valid opcode |
                    +-----------------+
```

## Prérequis

- Rust: [Installation](https://www.rust-lang.org/tools/install).
- Piston Window: Ajoutez `piston_window` à votre fichier `Cargo.toml`

## Lancement

```git
cd threads
cargo build
cargo run --bin server
cargo run --bin client
```
