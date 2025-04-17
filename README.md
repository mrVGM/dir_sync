## dir_sync

This is a tool for sending multiple files between computers in the same Local Area Network.
It is written in Rust and uses TCP connections for data transfering.
When tested in a wired LAN network with standard 1Gbps bandwidth, it reaches a data transfer speeds of
roughly 110 MB/s, which is close to the physical limit of the connection itself.

## How to Use

The app is just a simple executable named `ds`.
`ds` runs in two modes: **server** and **client**.
To open the app in **server** mode you just run
```
ds server
```
in the terminal. This will open a TCP socket and print out its IP address and port.
You need to run
```
ds client <IP address>:<port>
```
to connect to a server. Then the file transfer will begin automatically.
The app the send all the files from its current working directory to the current working directory of its counterpart.
This includes subdirectories too.
