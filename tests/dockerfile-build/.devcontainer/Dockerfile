ARG BASE_IMAGE=alpine:3.19
FROM ${BASE_IMAGE}

ARG BUILD_DATE
LABEL build_date="${BUILD_DATE}"

RUN apk add --no-cache git curl bash

WORKDIR /workspace

CMD ["/bin/bash"]
