FROM lukemathwalker/cargo-chef:latest-rust-1.68.2-slim AS chef
WORKDIR /app


FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder 
RUN apt update && apt install -y protobuf-compiler
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --package electricity


FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/electricity /
CMD ["/electricity"]

