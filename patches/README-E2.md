# E2 patches

These patches are experiment instrumentation, not transfer fixes.

Apply each from its upstream checkout with `git apply`.
`scripts/e2-real-transfer --build` applies them if they are not already applied.
It refuses a checkout at another commit.

- `e2-horizon-input-trace.patch` applies to Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`.
  It logs a timestamp, tick, position and server velocity after a movement packet passes the
  server's checks. It excludes teleport acknowledgements. The environment variable
  `E2_TRACE_INPUTS` enables it. Without that variable, it emits nothing.
- `e2-meridian-client.patch` applies to Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`.
  It adds one testkit binary, `e2-client`, and a dependency already present in the workspace.
  The binary uses the existing testkit login, speaks Minecraft 26.3, sends a position every
  50 ms, accepts absolute teleports and accepts commands on stdin. It reports timestamps,
  received position corrections and protocol connection byte counters. It does not implement
  Minecraft movement physics. Unexpected relative teleports or configuration restarts fail the run.

Horizon and Meridian were read at those commits, including their MIT LICENSE files.
No Folia, Paper or other GPL source, or `docs/research/05-folia.md`, was read.
The client follows Meridian's `meridian-testkit/src/live.rs` and `client.rs` and uses
Horizon protocol types. The server patch changes only the accepted-movement tracing point.
The proxy, sidecar and handoff state machine are unchanged.

## MIT notices

Copyright (c) 2026 Ludovic Marie and the Horizon contributors

Copyright (c) 2026 Akirabane

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
