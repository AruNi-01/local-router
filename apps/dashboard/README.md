# Dashboard

This package is the LocalRouter dashboard app.

For project setup, daemon/CLI usage, routing, manifest configuration, and development workflow, see the root guide:

- [LocalRouter Getting Started](../../README.md)

Local dashboard commands:

```bash
cd apps/dashboard
npm install
npm run dev
```

Optional API override:

```bash
VITE_LOCALROUTER_API=http://127.0.0.1:9731/v1 npm run dev
```
