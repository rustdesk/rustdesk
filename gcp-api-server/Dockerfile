# Use lightweight Node.js Alpine base image matching local environment
FROM node:22-alpine AS builder

WORKDIR /usr/src/app

# Install package dependencies
COPY package*.json ./
RUN npm ci

# Copy codebase and compile TypeScript files
COPY tsconfig.json ./
COPY src ./src
RUN npm run build

# Prune development dependencies to keep production image small
RUN npm prune --production

# ==========================================
# Production Image Setup
# ==========================================
FROM node:22-alpine

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/package*.json ./
COPY --from=builder /usr/src/app/node_modules ./node_modules
COPY --from=builder /usr/src/app/dist ./dist

# Standard Cloud Run port
ENV PORT=8080
EXPOSE 8080

CMD ["npm", "start"]
