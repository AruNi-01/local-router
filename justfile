set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
  @just --list

help:
  @just --list

backend *flags:
  if [[ " {{flags}} " == *" watch=true "* ]]; then exec cargo watch --watch apps/localrouterd --watch crates/localrouter-core -x 'run -p localrouterd'; else exec cargo run -p localrouterd; fi

frontend api_url="http://127.0.0.1:9731/v1" port="5173":
  cd apps/dashboard && VITE_LOCALROUTER_API="{{api_url}}" npm run dev -- --host 127.0.0.1 --port {{port}}

cli +args:
  cargo run -p localrouter-cli -- {{args}}

build:
  cargo build
  cd apps/dashboard && npm run build

check:
  cargo check
  cd apps/dashboard && npm run build

test:
  cargo test
  cd apps/dashboard && npm run test

fmt:
  cargo fmt --all

reset:
  cargo run -p localrouter-cli -- reset all
