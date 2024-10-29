# Contribute to Documents

This project's documentation is built using [mdBook](https://rust-lang.github.io/mdBook/).

## I18n (Internationalization)

This project's documentation uses [mdbook-i18n-helpers](https://github.com/google/mdbook-i18n-helpers) for internationalization.

Welcome to help us translate this project into your language!

### Initialize a New Translation

See [mdbook-i18n-helpers/USAGE.md ](https://github.com/google/mdbook-i18n-helpers/blob/main/i18n-helpers/USAGE.md)

First, you need to install [Gettext](https://www.gnu.org/software/gettext/). For Windows users, you can use [this](https://github.com/vslavik/gettext-tools-windows/releases).

Generate the `pot` template. (`messages.pot` doesn't need to be uploaded to the repo)

```
MDBOOK_OUTPUT='{"xgettext": {"depth": "1"}}' \
  mdbook build -d po/messages
```
Then, create a `po` file for your language. They are named after the [ISO 639](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) language codes: Danish would go into `po/da.po`, Korean would go into `po/ko.po`, etc.
```
msginit -i po/messages.pot -l xx -o po/xx.po
```
Next, add a `li` for your language in the `language-list` in `docs\theme\index.hbs`.
Like this:

```
<li role="none"><button role="menuitem" class="theme">
    <a id="zh_CN">Chinese Simplified (简体中文)</a>
</button></li>
```
Then, add your language code to `env:TRANSLATED_LANGUAGES` in `.github\workflows\docs.yml`.

### Continue Translating an Existing Translation

You can install a `.po` file editor, such as [Poedit](https://poedit.net/).
Then open the `.po` file and translate.
Currently, there's no need to compile to `.mo` files.

### Updating an Existing Translation

As the source text changes, translations gradually become outdated. To update the `po/xx.po` file with new messages, first extract the source text into a `po/messages.pot` template file. Then run

```
msgmerge --update po/xx.po po/messages.pot
```

Unchanged messages will stay intact, deleted messages are marked as old, and updated messages are marked "fuzzy". A fuzzy entry will reuse the previous translation: you should then go over it and update it as necessary before you remove the fuzzy marker.