const fs = require('fs');
const sharp = require('sharp');

const input = process.argv[2] || 'note/test.svg';
const output = process.argv[3] || 'svg_raster.h';
const width = Number(process.argv[4] || 191);
const height = Number(process.argv[5] || 142);

async function main() {
  const svg = fs.readFileSync(input);
  const { data, info } = await sharp(svg, { density: 96 })
    .resize(width, height, { fit: 'fill' })
    .raw()
    .toBuffer({ resolveWithObject: true });

  const pixels = [];
  for (let i = 0; i < data.length; i += info.channels) {
    const r = data[i];
    const g = data[i + 1];
    const b = data[i + 2];
    const a = info.channels > 3 ? data[i + 3] : 255;
    const argb = (((a << 24) | (r << 16) | (g << 8) | b) >>> 0);
    pixels.push(`0x${argb.toString(16).padStart(8, '0')}`);
  }

  const lines = [];
  lines.push('#ifndef SVG_RASTER_H');
  lines.push('#define SVG_RASTER_H');
  lines.push('');
  lines.push('#include <stdint.h>');
  lines.push('');
  lines.push(`#define SVG_RASTER_WIDTH ${width}`);
  lines.push(`#define SVG_RASTER_HEIGHT ${height}`);
  lines.push('');
  lines.push(`static const uint32_t svg_raster_pixels[${pixels.length}] = {`);
  for (let i = 0; i < pixels.length; i += 12) {
    lines.push(`  ${pixels.slice(i, i + 12).join(', ')},`);
  }
  lines.push('};');
  lines.push('');
  lines.push('#endif');

  fs.writeFileSync(output, lines.join('\n'));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
