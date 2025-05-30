# Website

This website is built using [VitePress](https://vitepress.dev/), which is a static site generator driven by Vite and Vue.

### Installation

#### Prerequisites

- [Node.js](https://nodejs.org/) version 18 or higher.
- Terminal for accessing VitePress via its command line interface (CLI).
- Text Editor with [Markdown](https://en.wikipedia.org/wiki/Markdown) syntax support.
  - [VSCode](https://code.visualstudio.com/) is recommended, along with the [official Vue extension](https://marketplace.visualstudio.com/items?itemName=Vue.volar).

Clone the repository:

```bash
git clone https://github.com/HaoboGu/rmk
cd ./docs
```

Install dependencies:

```sh [pnpm]
pnpm install
```

### Prettier

```sh [pnpm]
pnpm run docs:format
```

To format all documentation files,we use [Prettier](https://prettier.io/) to maintain consistent code style across documentation files.

### Local Development

Start the development server:

```sh [pnpm]
pnpm run docs:dev
```

This command will start the local development server and automatically open the browser window. Most changes will be reflected in real time without the need to restart the server.

### Build

Build the application:

```sh [pnpm]
pnpm run docs:build
```

This command generates static content into the `/docs/.vitepress/dist` directory and can be served using any static contents hosting service.
