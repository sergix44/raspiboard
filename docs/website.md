# Raspiboard website

The public landing page lives in `website/`. It uses plain HTML, CSS, and a small
JavaScript enhancement for copying installation commands. All content and links
work without JavaScript. There are no build dependencies, external fonts,
analytics, or runtime API requests. The static interface preview follows the
app's layout with generic example sounds; its controls do not play audio. It
uses the same bundled Monaspace Krypton font as the desktop app.

The page keeps the overview to three short features and downloads. Installation
commands and requirements are expandable; the user guide holds detailed usage.

## Files

- `website/index.html`: content, inline icons, and page metadata.
- `website/guide.html`: the complete user guide and its section navigation.
- `website/styles.css`: responsive layout and dark theme.
- `website/script.js`: optional clipboard buttons and their status messages.
- `img/icon_full.png`: shared project logo, copied into the published assets.
- `assets/fonts/MonaspaceKrypton-Regular-1.400.otf`: preview font, copied alongside
  its `monaspace-LICENSE` file.
- `.github/workflows/pages.yml`: stages and publishes the static page.

The website uses relative asset URLs so it works at the repository's
`/raspiboard/` path. Package links open the GitHub Releases list; they do not
depend on a particular version or a published release already existing. The
development-build link points to the existing build workflow.

## Local preview

From the project root, stage the same directory structure as the Pages workflow:

```sh
mkdir -p target/website/raspiboard/assets
cp website/index.html website/guide.html website/styles.css website/script.js target/website/raspiboard/
cp img/icon_full.png target/website/raspiboard/assets/
cp assets/fonts/MonaspaceKrypton-Regular-1.400.otf assets/fonts/monaspace-LICENSE target/website/raspiboard/assets/
python3 -m http.server 8080 --bind 127.0.0.1 --directory target/website
```

Open `http://127.0.0.1:8080/raspiboard/` and follow **User guide** to
`guide.html`. Stop the server with Ctrl+C. Re-run the
copy commands after editing. Check [website acceptance](acceptance.md#website)
at desktop and phone widths. Expect the dark page, app interface preview,
working navigation and release links. Copy buttons appear when you expand the
installation instructions. The browser
console should have no errors and every local asset should return HTTP 200.

For a JavaScript syntax check:

```sh
node --check website/script.js
```

## GitHub Pages setup

When ready to publish:

1. In the GitHub repository, open **Settings → Pages** and select
   **GitHub Actions** as the build and deployment source.
2. Publish the website files and workflow to `master`. A push changing
   `website/`, the shared logo, preview font/license, or the workflow triggers **website →
   publish-website**. Alternatively, manually run the **website** workflow on
   `master` once the files are on GitHub.
3. Expect a successful `github-pages` deployment at
   `https://sergix44.github.io/raspiboard/`. The workflow also reports the actual
   deployment URL. Manual runs on other branches skip deployment.

The workflow uploads only the website, shared logo, and preview font/license; app sources, saved data,
and package artifacts are not included. It uses GitHub's short-lived workflow
token and Pages identity token, so no additional repository secret is required.
The desktop packaging workflow runs separately.

See GitHub's [custom Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
for publishing-source and environment requirements.

## Content upkeep

The guide is maintained directly in `website/guide.html`; there is no separate
Markdown copy or documentation build step. Keep feature descriptions and system
requirements aligned with the [README](../README.md),
[user guide](../website/guide.html), and
[packaging guide](packaging.md). The installation examples assume one downloaded
package in the working folder. Verify `SHA256SUMS` from the same release before
running it. No APT repository is advertised until one is available.
