# Contributing to OpenPit

OpenPit welcomes focused, practical contributions that improve the SDK,
bindings, documentation, examples, or project tooling. No project membership
or prior approval is required to submit a pull request.

## Community Channels

- Use [GitHub Discussions](https://github.com/openpitkit/pit/discussions) for
  questions, design discussion, integration advice, and early proposals.
- Use [GitHub Issues](https://github.com/openpitkit/pit/issues) for confirmed
  bugs and scoped feature requests.
- If you want a small first task, start with
  [good first issues](https://github.com/openpitkit/pit/labels/good%20first%20issue).
- Report security vulnerabilities privately. Follow
  [SECURITY.md](SECURITY.md) instead of opening a public issue.

For anything non-trivial, open an issue or discussion before investing in an
implementation. This helps avoid duplicate work and keeps the design aligned
with the project scope.

## Prepare a Branch

1. Fork the repository and create a topic branch from the latest `main`.
2. Use a short, hyphenated, descriptive branch name. Include the related issue
   number when one exists, for example `191-add-community-docs`.
3. Keep the branch limited to one bug, feature, or documentation update.
4. Update the branch from `main` before requesting final review and resolve any
   conflicts without pulling unrelated changes into the pull request.

OpenPit does not require a particular personal or fork namespace. Internal
maintainer prefixes such as `wt/` are not required for external contributions.

## Build and Validate

Follow the [Local Build and Test](README.md#local-build-and-test) instructions
to install the required toolchains and create the repository-local Python
environment. From the repository root, a typical debug build and complete debug
gate are:

```bash
just build-debug
just check-debug
```

Use the narrowest gate that covers a focused language-only change:

```bash
just check-rust-debug
just check-go-debug
just check-python-debug
just check-cpp-debug
just check-js
```

Use `just check-full` for cross-cutting or release-critical changes that need
both debug and release validation. For Markdown-only changes, run
`markdownlint-cli2` on every changed Markdown file.

The check recipes may format files and regenerate API artifacts. Review those
changes and include generated files only when they belong to the contribution.

## Licensing

OpenPit requires neither a Contributor License Agreement nor Developer
Certificate of Origin sign-off. Contributions intentionally submitted to the
project are accepted under the repository's
[Apache License 2.0](LICENSE) on an inbound-equals-outbound basis unless the
contributor explicitly states otherwise.

## Pull Request Checklist

Before opening a pull request:

- Keep the change scoped to one bug, feature, or documentation update.
- Update public documentation and example mirrors together when changing public
  examples.
- Run the narrowest checks that match the files you changed.
- Include notable validation results in the pull request description.
- Link the related issue or discussion when one exists.
- Explain public API or behavior changes and call out compatibility risks.

Do not include credentials, private trading data, customer data, proprietary
business details, production logs, or other sensitive material in issues,
discussions, pull requests, or test fixtures.

## Review and Governance

Submitting a pull request is the path to becoming an OpenPit contributor. A
maintainer reviews the change for scope, correctness, tests, documentation, and
long-term maintainability. Address review comments with focused follow-up
commits or explain why a suggested change does not apply.

The `OWNERS` file records project ownership and the public maintainer contact.
Maintainers may delegate review, but an authorized maintainer makes the final
merge decision. Maintainer responsibilities may be offered to contributors who
show sustained, constructive participation and sound technical judgment.

Maintainers may decline changes that are out of scope, too broad to review
safely, not maintainable within the current project direction, or missing the
required tests and documentation updates.
