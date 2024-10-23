FROM rust:latest AS builder

WORKDIR /app

COPY . .

RUN cargo build --bin server --release

FROM debian

WORKDIR /app

COPY --from=builder /app/target/release/server .

EXPOSE 42428

CMD [ "./server" ]
