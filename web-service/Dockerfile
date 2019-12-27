ARG BASE_IMAGE=ekidd/rust-musl-builder:latest

# Our first FROM statement declares the build environment.
FROM ${BASE_IMAGE} AS builder

# Add our source code.
ADD . ./

# Fix permissions on source code.
RUN sudo chown -R rust:rust /home/rust

# Install mysql client lib for `diesel`
RUN sudo apt-get update && sudo apt-get install -y libmysqlclient-dev

# Build our application.
RUN cargo build --release

# Now, we need to build our _real_ Docker container, copying in `main`.
FROM alpine:latest
RUN apk --no-cache add ca-certificates
COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/main \
    /
CMD ["/main"]

EXPOSE 8090