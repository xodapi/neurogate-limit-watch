# Security

`neurogate-limit-watch` is intentionally small and local-first.

## Secrets

- Do not commit API keys.
- Pass the NeuroGate key through `NEUROGATE_API_KEY`.
- The tool never writes the key to disk.
- The tool does not print the key in normal errors.

## Network

The only network request is:

```text
GET https://api.neurogate.space/v1/me
Authorization: Bearer <api-key>
```

`--mock` performs no network requests.

## Reports

If you find a security issue, open a private report or contact the maintainer.

