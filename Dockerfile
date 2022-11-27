FROM rust:slim

WORKDIR /app

RUN apt-get update
RUN apt-get install -y libpq-dev # diesel_cli dependency
RUN cargo install diesel_cli --no-default-features --features postgres
RUN apt-get install -y libpoppler-glib-dev # poppler dependencies
# COPY . /app
COPY Cargo.toml  /app/Cargo.toml
COPY Cargo.lock  /app/Cargo.lock
COPY diesel.toml /app/diesel.toml
COPY src         /app/src
COPY migrations  /app/migrations
RUN cargo install --path .

CMD [ "indexer" "-s" "input/frenchST.txt" ]
