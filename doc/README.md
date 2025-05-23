# Website

This website is built using [VitePress](https://vitepress.dev/), which is a static site generator driven by Vite and Vue.

### Installation

#### Prerequisites

- [Node.js](https://nodejs.org/) version 18 or higher.
- Terminal for accessing VitePress via its command line interface (CLI).
- Text Editor with [Markdown](https://en.wikipedia.org/wiki/Markdown) syntax support.
  - [VSCode](https://code.visualstudio.com/) is recommended, along with the [official Vue extension](https://marketplace.visualstudio.com/items?itemName=Vue.volar).


1. Clone the repository:
   ```bash
   git clone https://github.com/HaoboGu/rmk
   cd ./doc
   ```
2. Install dependencies:
   ```sh [pnpm]
   pnpm install
   ```

### Local Development

3. Start the development server:
   ```sh [pnpm]
   pnpm run docs:dev
   ```


This command will start the local development server and automatically open the browser window. Most changes will be reflected in real time without the need to restart the server.
### Build

4. Build the application:
   ```sh [pnpm]
   pnpm run docs:build
   ```

This command generates static content into the `/doc/.vitepress/dist` directory and can be served using any static contents hosting service.