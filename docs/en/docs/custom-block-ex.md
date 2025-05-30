# Custom Block Ex

::: info 

This is an info box `code`.

```js
conslog('hello world')
```

:::

> [!NOTE] 
> this is a note `code`.
>```js
>conslog('hello world');
>```

::: tip 

This is a tip `code`.

```js
conslog('hello world')
```

:::

::: warning 

This is a warning `code`.

```js
conslog('hello world')
```

:::

::: danger 

This is a dangerous warning `code`.

```js
conslog('hello world')
```

:::

::: details 

This is a details block `code`.

```js
conslog('hello world')
```

:::

---

```js
conslog('hello world')
```

---

::: code-group

```js [config.js]
/**
 * @type {import('vitepress').UserConfig}
 */
const config = {
  // ...
}

export default config
```

```ts [config.ts]
import type { UserConfig } from 'vitepress'

const config: UserConfig = {
  // ...
}

export default config
```

:::