# Visualization performance budget

The public landing page is static and does not load the interactive React or
Three.js application. The demo loads the 3-D scene through a lazy chunk, so
Three.js is requested only after the demo application mounts.

`npm run performance:check` is enforced in visualization CI after the production
build. Its gzip budgets are:

| Artifact | Budget |
| --- | ---: |
| Landing `index.html` | 16 KiB |
| Initial demo JavaScript, excluding Three.js | 160 KiB |
| `three-vendor` chunk | 384 KiB |
| Total demo JavaScript | 512 KiB |

## Scaling behavior

- The public baseline is 25 crews, 35 stations, and 3 findings.
- The rendering path filters visible crew segments before drawing.
- Browsers without WebGL use the built-in interactive 2-D SVG route map instead
  of loading a failing canvas.
- A larger fixture must keep the same public fixture gate, be measured with the
  production budget script, and either remain within these limits or introduce
  an explicit reduced-detail mode before becoming the default demo.
