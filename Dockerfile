# Builder stage
FROM rust:1.75.0 AS builder 

WORKDIR /app
RUN apt update && apt install lld clang -y
COPY . .
ENV SQLX_OFFLINE true
RUN cargo build --release



# Runtime stage
FROM rust:1.75.0 AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/zero2prod zero2prod
COPY config config
ENV APP_ENV production
ENTRYPOINT ["./zero2prod"]
