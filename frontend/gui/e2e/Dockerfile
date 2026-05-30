FROM node:24

WORKDIR /tests

COPY package.json package-lock.json* ./
RUN npm install
RUN npx playwright install --with-deps chromium

COPY . .

CMD ["npx", "playwright", "test"]
