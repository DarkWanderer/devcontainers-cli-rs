# Build stage
FROM alpine:3.19 AS builder

RUN apk add --no-cache build-base

WORKDIR /build
RUN echo "Building application..." > build.log && \
    echo "Build complete" > built.txt

# Development stage
FROM alpine:3.19 AS development

RUN apk add --no-cache bash git curl

WORKDIR /app
COPY --from=builder /build/built.txt /app/

CMD ["/bin/bash"]

# Production stage (not used in test)
FROM alpine:3.19 AS production

WORKDIR /app
COPY --from=builder /build/built.txt /app/

CMD ["/bin/sh"]
