# Security

`neurogate-limit-watch` is intentionally small, local-first, and Rust-first.

## Secrets

- Do not commit API keys.
- Pass the NeuroGate key through `NEUROGATE_API_KEY`.
- The tool never writes the key to disk.
- The tool does not print the key in normal errors.
- Demo and mock modes do not require a key.

## Network

The only network request in normal mode is:

```text
GET https://api.neurogate.space/v1/me
Authorization: Bearer <api-key>
```

`--demo` and `--mock` perform no NeuroGate network requests.

## Reports

If you find a security issue, open a private report or contact the maintainer.
