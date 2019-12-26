FROM rustembedded/cross:arm-unknown-linux-gnueabi-0.1.16

RUN apt-get update && \
    apt-get install -y \
        llvm-dev \
        libclang-dev \
        libc6-dev-i386 \
        clang
