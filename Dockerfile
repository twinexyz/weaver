FROM alpine AS prover
RUN apk update

FROM alpine AS egressa
RUN apk update

FROM alpine AS scheduler
RUN apk update

