Security Policy

Security Maintainer

Project: HQ Launcher
Repository: https://github.com/p-asta/hq-launcher
Security Maintainer: "@p-asta" (https://github.com/p-asta)

HQ Launcher is an open-source desktop application built with Rust, Tauri, and React.

The project maintainer is responsible for coordinating security vulnerability reports, reviewing security-sensitive code changes, assessing potential security risks, and maintaining the project's security policies.

Security maintenance responsibilities include:

- Reviewing potential vulnerabilities in the application and its dependencies.
- Evaluating security risks associated with third-party modifications and downloaded resources.
- Reviewing software update mechanisms and file integrity verification.
- Investigating authentication and credential-handling vulnerabilities.
- Developing and reviewing security patches.
- Coordinating responsible vulnerability disclosure.
- Maintaining security-related documentation and reporting procedures.

Supported Versions

Security reports are accepted for the latest publicly released version of HQ Launcher.

Version| Supported
Latest stable release| Yes
Older releases| Best effort
Unreleased development builds| Best effort

Reporting a Vulnerability

If you discover a potential security vulnerability in HQ Launcher, please report it privately.

Preferred reporting method:

"Report a vulnerability through GitHub Security Advisories" (https://github.com/p-asta/hq-launcher/security/advisories/new)

Please include:

1. A description of the vulnerability.
2. Affected versions and components.
3. Steps to reproduce the issue.
4. Potential security impact.
5. Proof-of-concept information, if available.
6. Suggested mitigation or remediation, if known.

Please avoid publicly disclosing unpatched vulnerabilities before maintainers have had an opportunity to investigate and address them.

Vulnerability Response Process

The security maintainer aims to follow this process:

1. Triage: Review submitted reports and determine whether they describe a security issue.
2. Assessment: Evaluate severity, exploitability, and affected components.
3. Remediation: Develop and test appropriate fixes.
4. Release: Publish security fixes when available.
5. Disclosure: Coordinate public disclosure and publish advisories when appropriate.

Response times depend on maintainer availability and issue complexity. No fixed response or remediation deadline is guaranteed.

Security Scope

Security-relevant components include:

- Application update and distribution mechanisms.
- Game version installation and file management.
- Third-party mod installation and dependency handling.
- Authentication integrations and credential handling.
- Local file operations and filesystem permissions.
- Application-to-webview communication and JavaScript interfaces.
- External resource downloads and integrity verification.

Third-Party Components

HQ Launcher integrates with third-party software, services, and game modifications.

Vulnerabilities affecting third-party components should generally be reported to their respective maintainers.

Issues caused by HQ Launcher's integration with these components may be reported to this project.

Responsible Disclosure

We encourage responsible vulnerability research conducted on systems and software that researchers own or are explicitly authorized to test.

Researchers should avoid accessing other users' data, disrupting services, or performing unauthorized testing against third-party infrastructure.

Valid security reports may be acknowledged in a public advisory with the researcher's consent.

Security Advisories

Confirmed vulnerabilities may be documented through GitHub Security Advisories.

Advisories may include affected versions, technical impact, remediation information, and appropriate researcher attribution.

Policy Maintenance

This security policy is maintained by "@p-asta" (https://github.com/p-asta) and may be updated as the project's security processes evolve.
