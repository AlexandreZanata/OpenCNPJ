FROM golang:1.21-alpine AS builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache git

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /app/bin/api ./cmd/api/main.go
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /app/bin/importer ./cmd/importer/main.go

FROM alpine:latest

WORKDIR /app

# Install runtime dependencies
RUN apk --no-cache add ca-certificates tzdata

# Copy binaries from builder
COPY --from=builder /app/bin/api /app/api
COPY --from=builder /app/bin/importer /app/importer

# Copy migrations
COPY migrations /app/migrations

# Create data directory
RUN mkdir -p /app/data

EXPOSE 8080

CMD ["/app/api"]
