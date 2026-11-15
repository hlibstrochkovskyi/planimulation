# Star map

- Asset: `starmap-2020-galactic.jpg`, 4096×2048, derived from the linear OpenEXR map.
- Title: Deep Star Maps 2020, galactic coordinates (January 2021 correction).
- Credit: **NASA/Goddard Space Flight Center Scientific Visualization Studio. Gaia DR2: ESA/Gaia/DPAC.** Visualization by Ernie Wright.
- Source: https://svs.gsfc.nasa.gov/4851/
- Download: https://svs.gsfc.nasa.gov/vis/a000000/a004800/a004851/starmap_2020_4k_gal.exr
- Usage guidelines: https://www.nasa.gov/nasa-brand-center/images-and-media/
- Retrieved: 2026-10-16.
- Source SHA-256: `bce1d2c364720fd13ebbfaf1d444404a96b7cb4d7d8ddc88d44447358552361a`.
- Bundled JPEG SHA-256: `fa0d6d2f6f8698725e6627c2f13c3b458022ccf499818d1a4f452cb968e88f22`.

Conversion with ImageMagick, retaining the original plate carrée projection:

```sh
magick starmap_2020_4k_gal.exr -set colorspace RGB -colorspace sRGB -depth 8 -quality 94 starmap-2020-galactic.jpg
```

The conversion applies the standard linear-to-sRGB transfer and clips values outside the JPEG range. Runtime presentation reconstructs a camera ray per screen pixel, performs a spherical lookup with horizontal wrapping and base-level linear filtering, and adjusts orientation and exposure. Mipmaps are disabled because isotropic filtering produces excessive radial blur at spherical poles. No polar masking or AI image modification is applied. This catalogue visualization replaces the previous ESO/S. Brunier publication panorama.

The viewport displays a short credit when this map is visible; the dialog supplies the full credit, source, processing, and usage guidelines. The credit is included in current-view PNG captures. The map is bundled for offline use and does not imply NASA/ESA endorsement or a calibrated astronomical coordinate/time system for generated worlds. Constellation figure overlays are not used.
