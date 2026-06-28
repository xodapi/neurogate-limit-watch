# Security

`vimit` is intentionally small, local-first, and Rust-first.

## Secrets

- Do not commit API keys.
- Pass the VibeMode key through `VIBEMODE_API_KEY`.
- Or keep it in a local `.env` file next to the binary or working directory.
- The tool never writes the key to disk.
- The tool does not print the key in normal errors.
- `.env` is gitignored; only `.env.example` should be committed.
- Demo and mock modes do not require a key.

## Network

The only network request in normal mode is:

```text
GET https://api.vibemod.pro/v1/me
Authorization: Bearer <api-key>
```

`--demo` and `--mock` perform no VibeMode network requests.

## Reports

If you find a security issue, open a private report or contact the maintainer.
