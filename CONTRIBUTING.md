# Contributing to ReadMD

Thank you for improving ReadMD. This project prioritizes local-first Markdown reading and editing, non-destructive preview repair, broad platform compatibility and clear documentation.

## Code of conduct

Be patient with platform-specific reports, avoid personal comments and keep discussion focused on reproducible behavior.

## Ways to help

- Reproduce bugs on Windows, macOS, Linux, KylinOS or UOS.
- Improve English, Simplified Chinese, Traditional Chinese or Japanese documentation.
- Add regression tests for conversion, rendering, pagination, export or OCR workflows.
- Audit screenshots and examples for private paths, credentials or confidential content.
- Review translations for natural wording rather than literal machine translation.

## Development workflow

1. Fork the repository and create a feature branch from `main`.
2. Create an isolated environment and install the requirements for your platform:

   ```bash
   python -m venv .venv
   source .venv/bin/activate
   python -m pip install -r config/requirements.txt
   ```

   On Windows use:

   ```powershell
   python -m venv .venv
   .venv\Scripts\activate
   python -m pip install -r config/requirements.txt
   ```

3. Keep the change focused and preserve local-first behavior.
4. Run relevant tests before submitting:

   ```bash
   python -m unittest discover -s tests -p "test_*.py"
   ```

5. Start the app from source when a manual test is required:

   ```bash
   ./scripts/run.sh
   ```

6. Open a pull request using the repository template.

## Commit and pull request guidance

- Use one logical change per pull request.
- Describe the user-visible problem and the tested result.
- Include redacted logs or synthetic samples only.
- Update all affected language variants when changing user-facing text.
- Do not upload confidential documents, credentials or private paths.

## Reporting bugs and security issues

Use [bug report forms](.github/ISSUE_TEMPLATE/bug_report.yml) for reproducible defects and [Security Advisories](https://github.com/Natsummerance/readMD/security/advisories/new) for private vulnerability reports. General workflow questions belong in [Discussions](https://github.com/Natsummerance/readMD/discussions).

