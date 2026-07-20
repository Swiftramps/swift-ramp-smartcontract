# Contributing

Thank you for your interest!

## Steps

1. Fork the repo
2. Create a branch: `git checkout -b fix/my-change`
3. Commit and push
4. Open a Pull Request

## Code Style

Follow existing conventions. Add tests for new features.

## Contract test coverage

Every change to a public contract function must include tests for its successful path, authorization requirements, and relevant invalid inputs. State-changing functions should also verify their storage or token-balance effects.

Before opening a pull request, run:

```sh
cargo fmt --package swiftramp-swap -- --check
cargo test --workspace
```
