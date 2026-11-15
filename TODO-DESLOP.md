Prefix: DSLP
Next: 10

Deslop campaign backlog ([plan](https://github.com/laputa-systems/stuff/blob/main/docs/campaigns/deslop.md)). One agent works this file: take the lowest-numbered open ticket whose area does not start with `Needs owner ·`. After DSLP-5, work the open P1 and P2 findings, then fill in `## Report` below.

## Report

Not started.

## Tickets

### DSLP-5 · P2 · Dead code: tools and debloat

- **Area:** deslop · dead code
- **Where:** whole repository
- **Problem:** No dead-code scan has run.
- **Recommendation:** Run the deslop Dead code step. Accept: tools clean or allowlisted with reasons.
- **Fix with:** deslop
- **Effort:** S

### DSLP-6 · P2 · One idle connection stalls every other request

- **Area:** Serving
- **Where:** src/main.rs:849 (Server::handle reads with no timeout), src/main.rs:1095 (sequential accept loop)
- **Problem:** The accept loop handles one connection at a time and Server::handle blocks in stream.read with no timeout. A client that connects and sends nothing (a browser preconnect, a stalled tab) blocks every later request indefinitely. Reproduced: with one idle connection open, a second GET got no response within 3s.
- **Recommendation:** Bound the wait for a request (a read timeout that answers 408 or closes) and stop one connection from serializing the rest, for example a scoped thread per connection. Add a boundary test that holds an idle connection open and requires a second request to succeed.
- **Fix with:** lifecycle-concurrency
- **Effort:** S



