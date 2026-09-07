import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (name) => readFileSync(new URL(`../${name}`, import.meta.url), "utf8");

test("index.html declares the canonical URL", () => {
  assert.match(read("index.html"), /<link\s+rel="canonical"\s+href="https:\/\/copperroute\.net\/"\s*\/>/);
});

test("index.html has a meta description", () => {
  assert.match(read("index.html"), /<meta\s+name="description"\s+content="[^"]{40,160}"\s*\/>/);
});

test("index.html declares Open Graph title, description, url and type", () => {
  const html = read("index.html");
  for (const property of ["og:title", "og:description", "og:url", "og:type"]) {
    assert.match(html, new RegExp(`<meta\\s+property="${property}"\\s+content="[^"]+"\\s*\\/>`));
  }
});

test("robots.txt allows crawling and points at the sitemap", () => {
  const robots = read("robots.txt");
  assert.match(robots, /^User-agent: \*$/m);
  assert.match(robots, /^Allow: \/$/m);
  assert.match(robots, /^Sitemap: https:\/\/copperroute\.net\/sitemap\.xml$/m);
});

test("robots.txt keeps crawlers out of the heavy asset directories", () => {
  const robots = read("robots.txt");
  assert.match(robots, /^Disallow: \/examples\/$/m);
  assert.match(robots, /^Disallow: \/pkg\/$/m);
});

test("sitemap.xml lists the site root", () => {
  assert.match(read("sitemap.xml"), /<loc>https:\/\/copperroute\.net\/<\/loc>/);
});
