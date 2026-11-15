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

### DSLP-7 · P2 · Requests with more than 16 headers get 400 Bad Request

- **Area:** Serving
- **Where:** src/main.rs:862 (httparse header array of 16)
- **Problem:** Server::handle parses into a fixed array of 16 headers, and httparse returns TooManyHeaders for more, which srv answers with 400. Browsers send about 15 to 18 headers on a navigation, more with cookies set on localhost by other dev servers. Reproduced: a GET with 17 headers returned 400.
- **Recommendation:** Size the header array for real browser requests (64 or more within the existing 8 KiB request buffer), and add a boundary test with a browser-sized header set.
- **Fix with:** none
- **Effort:** S


