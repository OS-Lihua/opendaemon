## Scripts

This directory contains utility scripts for building, testing, and release
automation.

### update-readme-version.sh

Updates version references in README files.

```bash
./scripts/update-readme-version.sh <new-version>
```

- Automatically updates version references in README files.
- Keeps documentation versions consistent.
- Requires `python3`; override with the `PYTHON_BIN` environment variable.
