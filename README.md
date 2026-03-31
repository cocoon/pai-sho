# pai-sho

P2P TCP port forwarding over [iroh](https://github.com/n0-computer/iroh).

My workflow is generally a dedicated VM per task -- a [vibenv](https://github.com/cablehead/vibenv.dag). These environments tend to have no open inbound ports. iroh's [dumbpipe](https://github.com/n0-computer/dumbpipe) worked nicely for reaching a single port, but when you need 3-4 ports you have to run 3-4 dumbpipes on each side -- so 6-8 processes just to share a few ports. pai-sho gives you a single long-lived daemon that manages multiple ports. You can expose and unexpose them on the fly, and if the connection drops it comes back on its own.

## Example

Say I have a VM running an [http-nu](https://github.com/cablehead/http-nu) app on `:3001` and [stellar](https://github.com/cablehead/stellar) on `:7331` for live CSS editing. I start the daemon with both ports exposed:

```sh
pai-sho daemon -e 3001 -e 7331
# Ticket: 5hc4bjqfp6booceusm3jrfebbegyfi6aiqwbgx4xxqmpvg5usoyq
```

On my laptop, I connect with the ticket:

```sh
pai-sho daemon -a 5hc4bjqfp6booceusm3jrfebbegyfi6aiqwbgx4xxqmpvg5usoyq
```

Now `localhost:3001` and `localhost:7331` on my laptop reach the VM. Close the laptop, reopen it -- the connection restores on its own.

Later, I spin up something new on the VM:

```sh
http-nu :3002 -c '{|req| "hello from a new experiment"}'
pai-sho expose 3002
```

It's immediately there at `http://localhost:3002` in my local browser. Done with it? `pai-sho unexpose 3002`.

## Install

```sh
cargo install pai-sho
```

```sh
brew install cablehead/tap/pai-sho
```

```sh
eget cablehead/pai-sho
```

Or grab a binary from [releases](https://github.com/cablehead/pai-sho/releases).

## Usage

```
pai-sho [--socket <path>] <command>
```

### Commands

```
daemon [options]        Start the daemon
ticket                  Print daemon's ticket
add-peer <ticket>       Connect to a peer
remove-peer <ticket>    Disconnect from a peer
expose <port>           Expose a local port to peers
unexpose <port>         Stop exposing a port
list                    Show peers, exposed ports, bindings
```

### Daemon Options

| Option | Default | Description |
|--------|---------|-------------|
| `--host` | `127.0.0.1` | Address to forward exposed ports to |
| `-a, --add` | | Add peer on startup (repeatable) |
| `-e, --expose` | | Expose port on startup (repeatable) |
| `--socket` | `/tmp/pai-sho.sock` | Unix socket path |

## How it works

Each daemon gets a unique ticket (an iroh endpoint ID). When you add a peer by ticket, iroh handles discovery and NAT traversal -- connecting directly when possible, falling back through relay servers when needed.

Exposed ports are announced to peers automatically. When a peer exposes port 3001, a local TCP listener binds `127.0.0.1:3001` on your side. Traffic goes over an encrypted QUIC connection.

If the connection drops, both sides reconnect with exponential backoff. Existing port bindings stay active and resume when the connection comes back.

## See also

[ngrok](https://ngrok.com) and [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/) are great when you need a public URL others can reach. pai-sho is for connecting your own machines, not exposing services to the internet.

[SSH tunnels](https://www.ssh.com/academy/ssh/tunneling) need inbound access on at least one side. pai-sho works when neither machine has open inbound ports.

[Tailscale](https://tailscale.com) and [ZeroTier](https://www.zerotier.com) give every machine an IP on a virtual network. pai-sho is narrower -- you expose specific ports, not your whole machine. Sometimes that's all you need.

[dumbpipe](https://github.com/n0-computer/dumbpipe) is the direct inspiration. It pipes a single TCP port between two peers. pai-sho builds on the same iroh foundation but manages multiple ports, multiple peers, and auto-reconnects.
