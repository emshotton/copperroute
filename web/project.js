// Translate routing net classes as well as invoking the Rust project DRC importer.
export function applyProject(input, text) {
  if (!text) return;
  const project = JSON.parse(text);
  if (!project.board?.design_settings)
    throw Error("Project has no board.design_settings.");
  const minimum = project.board.design_settings.rules ?? {};
  const fallback = input.board.netClasses[0];
  const source = project.net_settings?.classes ?? [];
  const classes = source.map((c) => ({
    name: c.name,
    clearance: Math.max(
      c.clearance ?? fallback.clearance,
      minimum.min_clearance ?? 0,
    ),
    traceWidth: Math.max(
      c.track_width ?? fallback.traceWidth,
      minimum.min_track_width ?? 0,
    ),
    viaDiameter: Math.max(
      c.via_diameter ?? fallback.viaDiameter,
      minimum.min_via_diameter ?? 0,
    ),
    viaDrill: Math.max(
      c.via_drill ?? fallback.viaDrill,
      minimum.min_through_hole_diameter ?? 0,
    ),
    netNames: [],
  }));
  if (!classes.some((c) => c.name === "Default"))
    classes.unshift({ ...fallback, netNames: [] });
  for (const c of classes) {
    if (
      !c.name ||
      ["clearance", "traceWidth", "viaDiameter", "viaDrill"].some(
        (key) => !Number.isFinite(c[key]) || c[key] <= 0,
      ) ||
      c.viaDrill >= c.viaDiameter
    )
      throw Error(`Invalid dimensions for project net class ${c.name}`);
  }
  const assignments = project.net_settings?.netclass_assignments ?? {};
  const patterns = project.net_settings?.netclass_patterns ?? [];
  const matches = (pattern, name) => {
    // Support KiCad's common whole-name wildcard patterns, fail on richer syntax.
    if (/[\[\]{}]/.test(pattern))
      throw Error(`Unsupported project net pattern: ${pattern}`);
    const regex = pattern
      .split("")
      .map((c) =>
        c === "*"
          ? ".*"
          : c === "?"
            ? "."
            : c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
      )
      .join("");
    return new RegExp(`^${regex}$`).test(name);
  };
  for (const net of input.board.nets) {
    let names =
      assignments[net.name] ??
      patterns
        .filter((p) => matches(p.pattern, net.name))
        .map((p) => p.netclass);
    if (!Array.isArray(names)) names = [names];
    names = [...new Set(names)].filter(Boolean);
    if (names.length > 1)
      throw Error(
        `Composite net classes for ${net.name} are not supported yet.`,
      );
    const name = names[0] ?? "Default";
    const cls = classes.find((c) => c.name === name);
    if (!cls) throw Error(`Unknown project net class: ${name}`);
    net.className = name;
    cls.netNames.push(net.name);
  }
  input.board.netClasses = classes;
  input.board.outline.clearance = Math.max(
    input.board.outline.clearance,
    minimum.min_copper_edge_clearance ?? 0,
  );
  input.warnings.push(
    "Project net classes and supported DRC constraints imported. Custom .kicad_dru rules and local pad overrides are not applied.",
  );
}
