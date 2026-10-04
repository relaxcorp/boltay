# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## What is signed

The Windows installer `Boltay_*_x64-setup.exe` and the application inside it. Both are built by [GitHub Actions](.github/workflows/release.yml) from a tagged commit of this repository; nothing built elsewhere gets signed. The macOS and Linux packages are not part of this.

## Team

| Role | Members |
| --- | --- |
| Committers and reviewers | [relaxcorp](https://github.com/relaxcorp) |
| Approvers | [relaxcorp](https://github.com/relaxcorp) |

Every change reaches `main` through a pull request that passes CI. A release is signed only after an approver has checked its draft. All members use two-factor authentication.

## Privacy

Boltay recognizes and translates speech on your computer. What you say, type, translate or transcribe, and any data about how you use the app, never leave it.

The app goes online for two things only:

- **Models.** When you pick a language or translate for the first time, it downloads the model files from the `models-v1` release of this repository, or from Hugging Face if that fails.
- **Updates.** Once a day it asks `api.github.com` for the number of the latest release and shows a link if there is a newer one. Turn it off with "Check for updates" in Settings → General.
