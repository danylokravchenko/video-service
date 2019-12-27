# ------------------------------------------------------------------------------
# Cargo Build Stage
# ------------------------------------------------------------------------------

FROM rust:latest as cargo-build

RUN apt-get update

RUN apt-get install musl-tools -y

ENV PKG_CONFIG_ALLOW_CROSS=1

COPY ./ ./

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --target x86_64-unknown-linux-musl --release

RUN mkdir -p /build-out

RUN cp target/x86_64-unknown-linux-musl/release/main /build-out/

RUN ls /build-out/

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM scratch

COPY --from=cargo-build /build-out/main /

CMD ["/main"]

EXPOSE 8091