smoketest:
    cargo xtask cef-smoketest

smoketest-selftest:
    cargo xtask cef-smoketest --selftest

precommit:
    make pre-commit

test:
    make test

build:
    make build
