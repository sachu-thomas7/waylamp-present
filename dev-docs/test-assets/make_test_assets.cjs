// Generates dev-docs/test-assets/streamdeck-test-deck.{pptx,pdf} and a sample scene image.
// Hand-rolled ZIP (for pptx) and PDF writers — no external deps, deterministic byte offsets.
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const OUT_DIR = __dirname;
fs.mkdirSync(OUT_DIR, { recursive: true });

// ---------- CRC32 ----------
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xFFFFFFFF;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xFF] ^ (c >>> 8);
  return (c ^ 0xFFFFFFFF) >>> 0;
}

// ---------- Minimal ZIP writer (store or deflate) ----------
function buildZip(entries) {
  // entries: [{ name: string, data: Buffer }]
  const localParts = [];
  const centralParts = [];
  let offset = 0;

  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, 'utf8');
    const compressed = zlib.deflateRawSync(data);
    const crc = crc32(data);
    const method = 8; // deflate

    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);   // version needed
    localHeader.writeUInt16LE(0, 6);    // flags
    localHeader.writeUInt16LE(method, 8);
    localHeader.writeUInt16LE(0, 10);   // mod time
    localHeader.writeUInt16LE(0, 12);   // mod date
    localHeader.writeUInt32LE(crc, 14);
    localHeader.writeUInt32LE(compressed.length, 18);
    localHeader.writeUInt32LE(data.length, 22);
    localHeader.writeUInt16LE(nameBuf.length, 26);
    localHeader.writeUInt16LE(0, 28);

    const localEntry = Buffer.concat([localHeader, nameBuf, compressed]);
    localParts.push(localEntry);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);  // version made by
    centralHeader.writeUInt16LE(20, 6);  // version needed
    centralHeader.writeUInt16LE(0, 8);   // flags
    centralHeader.writeUInt16LE(method, 10);
    centralHeader.writeUInt16LE(0, 12);
    centralHeader.writeUInt16LE(0, 14);
    centralHeader.writeUInt32LE(crc, 16);
    centralHeader.writeUInt32LE(compressed.length, 20);
    centralHeader.writeUInt32LE(data.length, 24);
    centralHeader.writeUInt16LE(nameBuf.length, 28);
    centralHeader.writeUInt16LE(0, 30);  // extra len
    centralHeader.writeUInt16LE(0, 32);  // comment len
    centralHeader.writeUInt16LE(0, 34);  // disk number
    centralHeader.writeUInt16LE(0, 36);  // internal attrs
    centralHeader.writeUInt32LE(0, 38);  // external attrs
    centralHeader.writeUInt32LE(offset, 42);

    centralParts.push(Buffer.concat([centralHeader, nameBuf]));
    offset += localEntry.length;
  }

  const centralDir = Buffer.concat(centralParts);
  const centralDirOffset = offset;
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(0, 4);
  eocd.writeUInt16LE(0, 6);
  eocd.writeUInt16LE(entries.length, 8);
  eocd.writeUInt16LE(entries.length, 10);
  eocd.writeUInt32LE(centralDir.length, 12);
  eocd.writeUInt32LE(centralDirOffset, 16);
  eocd.writeUInt16LE(0, 20);

  return Buffer.concat([...localParts, centralDir, eocd]);
}

// ---------- Minimal solid-color PNG ----------
function buildPng(width, height, [r, g, b]) {
  function chunk(type, data) {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length, 0);
    const typeData = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(typeData), 0);
    return Buffer.concat([len, typeData, crc]);
  }
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;  // bit depth
  ihdr[9] = 2;  // color type RGB
  ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;

  const rowBytes = width * 3;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y++) {
    const rowStart = y * (rowBytes + 1);
    raw[rowStart] = 0; // filter: none
    for (let x = 0; x < width; x++) {
      const px = rowStart + 1 + x * 3;
      raw[px] = r; raw[px + 1] = g; raw[px + 2] = b;
    }
  }
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([sig, chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0))]);
}

// ---------- Minimal multi-page PDF with real extractable text ----------
function buildPdf(pages) {
  // pages: [{ title: string, body: string[] }]
  const objects = [];
  function addObject(body) { objects.push(body); return objects.length; } // 1-indexed

  const catalogNum = 1;
  const pagesNum = 2;
  const fontNum = 3;
  const firstContentNum = 4;
  const firstPageNum = firstContentNum + pages.length;

  const pageNums = pages.map((_, i) => firstPageNum + i);
  const contentNums = pages.map((_, i) => firstContentNum + i);

  objects[catalogNum - 1] = `<< /Type /Catalog /Pages ${pagesNum} 0 R >>`;
  objects[pagesNum - 1] = `<< /Type /Pages /Kids [${pageNums.map((n) => `${n} 0 R`).join(' ')}] /Count ${pages.length} >>`;
  objects[fontNum - 1] = `<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>`;

  pages.forEach((p, i) => {
    const lines = [];
    lines.push(`BT /F1 24 Tf 72 700 Td (${escapePdf(p.title)}) Tj ET`);
    let y = 650;
    for (const line of p.body) {
      lines.push(`BT /F1 14 Tf 72 ${y} Td (${escapePdf(line)}) Tj ET`);
      y -= 24;
    }
    const stream = lines.join('\n');
    objects[contentNums[i] - 1] = `<< /Length ${Buffer.byteLength(stream, 'utf8')} >>\nstream\n${stream}\nendstream`;
  });

  pages.forEach((p, i) => {
    objects[pageNums[i] - 1] =
      `<< /Type /Page /Parent ${pagesNum} 0 R /MediaBox [0 0 612 792] ` +
      `/Resources << /Font << /F1 ${fontNum} 0 R >> >> /Contents ${contentNums[i]} 0 R >>`;
  });

  let out = '%PDF-1.4\n';
  const offsets = [0]; // object 0 unused (free list head)
  for (let i = 0; i < objects.length; i++) {
    offsets.push(Buffer.byteLength(out, 'utf8'));
    out += `${i + 1} 0 obj\n${objects[i]}\nendobj\n`;
  }
  const xrefOffset = Buffer.byteLength(out, 'utf8');
  out += `xref\n0 ${objects.length + 1}\n`;
  out += '0000000000 65535 f \n';
  for (let i = 1; i <= objects.length; i++) {
    out += `${String(offsets[i]).padStart(10, '0')} 00000 n \n`;
  }
  out += `trailer\n<< /Size ${objects.length + 1} /Root ${catalogNum} 0 R >>\n`;
  out += `startxref\n${xrefOffset}\n%%EOF`;

  return Buffer.from(out, 'latin1');
}
function escapePdf(s) {
  return String(s).replace(/\\/g, '\\\\').replace(/\(/g, '\\(').replace(/\)/g, '\\)');
}

// ================= Build the PPTX =================
const XML_HEADER = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n';

const contentTypes = XML_HEADER + `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/slides/slide2.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/slides/slide3.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/slides/slide4.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/slides/slide5.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>`;

const rootRels = XML_HEADER + `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>`;

const coreProps = XML_HEADER + `<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>StreamDeck Test Deck</dc:title>
  <dc:creator>StreamDeck</dc:creator>
</cp:coreProperties>`;

const appProps = XML_HEADER + `<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>StreamDeck Test Generator</Application>
  <Slides>5</Slides>
</Properties>`;

const presentationXml = XML_HEADER + `<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId2"/>
    <p:sldId id="257" r:id="rId3"/>
    <p:sldId id="258" r:id="rId4"/>
    <p:sldId id="259" r:id="rId5"/>
    <p:sldId id="260" r:id="rId6"/>
  </p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>`;

const presentationRels = XML_HEADER + `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide3.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide4.xml"/>
  <Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide5.xml"/>
  <Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
</Relationships>`;

const slideMaster = XML_HEADER + `<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>`;

const slideMasterRels = XML_HEADER + `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>`;

const slideLayout = XML_HEADER + `<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="title" preserve="1">
  <p:cSld name="Title Layout"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
  <p:clrMapOvr><a:overrideClrMapping bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>
</p:sldLayout>`;

const slideLayoutRels = XML_HEADER + `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>`;

const theme = XML_HEADER + `<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="StreamDeck Test Theme">
  <a:themeElements>
    <a:clrScheme name="StreamDeck">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="1F2937"/></a:dk2>
      <a:lt2><a:srgbClr val="E5E7EB"/></a:lt2>
      <a:accent1><a:srgbClr val="2563EB"/></a:accent1>
      <a:accent2><a:srgbClr val="7C3AED"/></a:accent2>
      <a:accent3><a:srgbClr val="059669"/></a:accent3>
      <a:accent4><a:srgbClr val="D97706"/></a:accent4>
      <a:accent5><a:srgbClr val="DC2626"/></a:accent5>
      <a:accent6><a:srgbClr val="0891B2"/></a:accent6>
      <a:hlink><a:srgbClr val="2563EB"/></a:hlink>
      <a:folHlink><a:srgbClr val="7C3AED"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="StreamDeck">
      <a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="StreamDeck">
      <a:fillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:fillStyleLst>
      <a:lnStyleLst>
        <a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
      </a:lnStyleLst>
      <a:effectStyleLst>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
      </a:effectStyleLst>
      <a:bgFillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>`;

function titleShape(text) {
  return `<p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
        <p:spPr/>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>${text}</a:t></a:r></a:p></p:txBody>
      </p:sp>`;
}
function subtitleShape(text) {
  // Unsupported placeholder type on purpose — exercises layout_unsupported.
  return `<p:sp>
        <p:nvSpPr><p:cNvPr id="4" name="Subtitle 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="subTitle" idx="1"/></p:nvPr></p:nvSpPr>
        <p:spPr/>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>${text}</a:t></a:r></a:p></p:txBody>
      </p:sp>`;
}
function bodyShape(paragraphsXml) {
  return `<p:sp>
        <p:nvSpPr><p:cNvPr id="3" name="Body 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
        <p:spPr/>
        <p:txBody><a:bodyPr/><a:lstStyle/>${paragraphsXml}</p:txBody>
      </p:sp>`;
}
function plainParagraph(text) {
  return `<a:p><a:r><a:t>${text}</a:t></a:r></a:p>`;
}
function monospaceParagraph(text) {
  return `<a:p><a:r><a:rPr lang="en-US" sz="1400"><a:latin typeface="Consolas"/></a:rPr><a:t>${text}</a:t></a:r></a:p>`;
}
function pictureShape() {
  return `<p:pic>
        <p:nvPicPr><p:cNvPr id="5" name="Picture 1"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
        <p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
        <p:spPr><a:xfrm><a:off x="2000000" y="2000000"/><a:ext cx="4000000" cy="2400000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
      </p:pic>`;
}
function slideXml(shapesXml) {
  return XML_HEADER + `<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      ${shapesXml}
  </p:spTree></p:cSld>
  <p:clrMapOvr><a:overrideClrMapping bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>
</p:sld>`;
}
function slideRels(extra) {
  return XML_HEADER + `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  ${extra || ''}
</Relationships>`;
}

const slide1 = slideXml(titleShape('StreamDeck Test Deck') + bodyShape(plainParagraph('A sample presentation for testing deck import.')));
const slide2 = slideXml(titleShape('Feature Overview') + bodyShape(
  plainParagraph('Live presentation control from the Controller window') +
  plainParagraph('Multi-camera production (Phase 2, not yet built)') +
  plainParagraph('One-click live streaming egress (Phase 4, not yet built)')
));
const slide3 = slideXml(titleShape('Sample Code') + bodyShape(monospaceParagraph('fn main() { println!("Hello, StreamDeck!"); }')));
const slide4 = slideXml(titleShape('Visual Slide') + pictureShape());
const slide5 = slideXml(titleShape('Unsupported Layout Demo') + subtitleShape('This uses a subtitle placeholder StreamDeck does not classify as body text.'));

const slide4Rels = slideRels('<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>');
const plainSlideRels = slideRels('');

const image1 = buildPng(640, 360, [37, 99, 235]); // matches theme accent1 blue

const entries = [
  { name: '[Content_Types].xml', data: Buffer.from(contentTypes, 'utf8') },
  { name: '_rels/.rels', data: Buffer.from(rootRels, 'utf8') },
  { name: 'docProps/core.xml', data: Buffer.from(coreProps, 'utf8') },
  { name: 'docProps/app.xml', data: Buffer.from(appProps, 'utf8') },
  { name: 'ppt/presentation.xml', data: Buffer.from(presentationXml, 'utf8') },
  { name: 'ppt/_rels/presentation.xml.rels', data: Buffer.from(presentationRels, 'utf8') },
  { name: 'ppt/slideMasters/slideMaster1.xml', data: Buffer.from(slideMaster, 'utf8') },
  { name: 'ppt/slideMasters/_rels/slideMaster1.xml.rels', data: Buffer.from(slideMasterRels, 'utf8') },
  { name: 'ppt/slideLayouts/slideLayout1.xml', data: Buffer.from(slideLayout, 'utf8') },
  { name: 'ppt/slideLayouts/_rels/slideLayout1.xml.rels', data: Buffer.from(slideLayoutRels, 'utf8') },
  { name: 'ppt/theme/theme1.xml', data: Buffer.from(theme, 'utf8') },
  { name: 'ppt/slides/slide1.xml', data: Buffer.from(slide1, 'utf8') },
  { name: 'ppt/slides/_rels/slide1.xml.rels', data: Buffer.from(plainSlideRels, 'utf8') },
  { name: 'ppt/slides/slide2.xml', data: Buffer.from(slide2, 'utf8') },
  { name: 'ppt/slides/_rels/slide2.xml.rels', data: Buffer.from(plainSlideRels, 'utf8') },
  { name: 'ppt/slides/slide3.xml', data: Buffer.from(slide3, 'utf8') },
  { name: 'ppt/slides/_rels/slide3.xml.rels', data: Buffer.from(plainSlideRels, 'utf8') },
  { name: 'ppt/slides/slide4.xml', data: Buffer.from(slide4, 'utf8') },
  { name: 'ppt/slides/_rels/slide4.xml.rels', data: Buffer.from(slide4Rels, 'utf8') },
  { name: 'ppt/slides/slide5.xml', data: Buffer.from(slide5, 'utf8') },
  { name: 'ppt/slides/_rels/slide5.xml.rels', data: Buffer.from(plainSlideRels, 'utf8') },
  { name: 'ppt/media/image1.png', data: image1 },
];

const pptxBuf = buildZip(entries);
fs.writeFileSync(path.join(OUT_DIR, 'streamdeck-test-deck.pptx'), pptxBuf);
console.log('wrote pptx:', pptxBuf.length, 'bytes,', entries.length, 'entries');

// ================= Build the PDF =================
const pdfBuf = buildPdf([
  { title: 'StreamDeck Test PDF', body: ['Page 1 of 3 - manual deck import validation.'] },
  { title: 'Second Page', body: ['Each PDF page becomes one rasterized slide.', 'Title = first line of extracted text.'] },
  { title: 'Third Page', body: ['Background image is the full rendered page.'] },
]);
fs.writeFileSync(path.join(OUT_DIR, 'streamdeck-test-deck.pdf'), pdfBuf);
console.log('wrote pdf:', pdfBuf.length, 'bytes');

// ================= Sample scene-media image =================
const sceneImg = buildPng(640, 360, [220, 38, 38]); // red, visually distinct from slide background
fs.writeFileSync(path.join(OUT_DIR, 'streamdeck-test-scene-image.png'), sceneImg);
console.log('wrote scene image:', sceneImg.length, 'bytes');
