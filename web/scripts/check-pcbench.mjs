import { chromium } from '@playwright/test';
import { readFileSync, readdirSync, mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
import { importBoard, parse } from '../kicad.js';
import { previewBoard } from '../preview.js';
import { applyProject } from '../project.js';
const root = resolve(process.argv[2]);
const output = resolve(process.argv[3] ?? '/tmp/pcbench-browser-check');
mkdirSync(output, {recursive: true});
const rules = {traceWidth:.25, clearance:.2, viaDiameter:.6, viaDrill:.3};
const browser = await chromium.launch({headless:true});
const reports = [];
try {
  for (const name of readdirSync(root).filter(n => n.startsWith('pcbench-') && (!process.argv[4] || new RegExp(process.argv[4]).test(n))).sort()) {
    const folder = join(root, name), report = {name};
    console.log(`${name}: checking import and preview`);
    try {
      for (const file of ['processed.kicad_pcb', 'raw.kicad_pcb']) {
        const text = readFileSync(join(folder,file),'utf8');
        assert.ok(previewBoard(text).includes('<svg'));
        const input = importBoard(text,file,rules,{ripUpRouting:true,rebuildZones:true});
        if(file === "processed.kicad_pcb") report.nets = input.board.nets.length;
        report.pads = input.board.components.length;
        report.cutouts = input.board.outline.cutouts.length;
        if(existsSync(join(folder,'raw.kicad_pro'))) applyProject(input,readFileSync(join(folder,'raw.kicad_pro'),'utf8'));
      }
      const page = await browser.newPage();
      page.on("console", message => {if(message.type() === "error") console.error(name, message.text());});
      try {
        await page.goto('http://127.0.0.1:8080');
        const files = [join(folder,'processed.kicad_pcb')];
        if(existsSync(join(folder,'raw.kicad_pro'))) files.push(join(folder,'raw.kicad_pro'));
        await page.locator('#file').setInputFiles(files);
        await page.waitForFunction(() => document.querySelector('#status').textContent.includes('Board selected.'));
        assert.ok(await page.locator('#preview svg').count());
        await page.locator('#seconds').fill('60');
        await page.locator('#passes').fill('5');
        console.log(`${name}: routing in browser`);
        await page.locator('#route').click();
        await page.waitForFunction(() => !document.querySelector('#pcb').hidden || document.querySelector('#badge').textContent === 'Could not route',null,{timeout:180000});
        report.status = await page.locator('#status').textContent();
        assert.equal(await page.locator('#pcb').isVisible(),true,report.status);
        const download = page.waitForEvent('download');
        await page.locator('#pcb').click();
        const file = await download;
        const pcb = readFileSync(await file.path(),'utf8');
        writeFileSync(join(output,name+'.kicad_pcb'),pcb);
        const routed = importBoard(pcb,'routed',rules,{rebuildZones:true});
        const inside = (p, ps) => {
          let result = false;
          for(let i=0,j=ps.length-1;i<ps.length;j=i++) {
            const a=ps[i],b=ps[j];
            if((a.y>p.y)!==(b.y>p.y) && p.x<(b.x-a.x)*(p.y-a.y)/(b.y-a.y)+a.x) result=!result;
          }
          return result;
        };
        const cross = (a,b,c) => (b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);
        const crosses = (a,b,c,d) => cross(a,b,c)*cross(a,b,d)<-1e-12 && cross(c,d,a)*cross(c,d,b)<-1e-12;
        for(const polygon of routed.board.outline.cutouts) {
          assert.ok(routed.board.vias.every(v=>!inside(v.position,polygon)), 'Via inside a cutout');
          for(const t of routed.board.traces) for(let i=1;i<t.points.length;i++) {
            const a=t.points[i-1],b=t.points[i];
            assert.ok(!inside(a,polygon)&&!inside(b,polygon), 'Track inside a cutout');
            assert.ok(polygon.every((p,j)=>!crosses(a,b,p,polygon[(j+1)%polygon.length])), 'Track crosses a cutout');
          }
        }
        report.traces = routed.board.traces.length;
        report.vias = routed.board.vias.length;
        assert.ok(report.traces > 0);
        const original = readFileSync(join(folder,'processed.kicad_pcb'),'utf8');
        const retained = text => parse(text).values.filter(n => n?.values && !['segment','via','arc','zone'].includes(n.values[0])).map(n=>text.slice(n.start,n.end));
        assert.deepEqual(retained(pcb),retained(original));
        const svgDownload = page.waitForEvent('download');
        await page.locator('#svg').click();
        const svg = readFileSync(await (await svgDownload).path(),'utf8');
        assert.ok(svg.includes('<svg'));
        writeFileSync(join(output,name+'.svg'),svg);
        report.warnings = await page.locator('#warnings').textContent();
        report.ok = true;
      } finally {await page.close();}
    } catch(error) {report.ok=false;report.error=error.message;}
    reports.push(report);
    writeFileSync(join(output,'report.json'),JSON.stringify(reports,null,2));
    console.log(JSON.stringify(report));
  }
} finally {await browser.close();}
if(reports.some(r=>!r.ok)) process.exitCode=1;
