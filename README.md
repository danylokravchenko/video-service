# Notice: It's only proof of concept that shows that Rust async runtime allows to easily build complex, safe and fast applications with high performance

## This application contains async web server with async database that communicates with user and video service that stores, consumes and does its black magic with videos. It allows to handle lots of connections at the time and do not block while video service will process uploaded videos

## Both web server and video service are built using async tokio ecosystem and communicate with each other using streams

## It allows to get rid of blocking operations and achieve the best performance from the hardware and reduce amount of RAM that is needed to run all of this
