import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ShellSurfaceProvider, useShellSurface } from "./ShellSurfaceContext";

function SurfaceProbe() {
  const { visibility } = useShellSurface();
  return <span data-visibility={visibility}>{visibility}</span>;
}

describe("ShellSurfaceContext", () => {
  test("defaults isolated surfaces to visible", () => {
    const markup = renderToStaticMarkup(<SurfaceProbe />);

    assert.match(markup, /data-visibility="visible"/);
    assert.match(markup, />visible</);
  });

  test("provides shell-owned visibility to route surfaces", () => {
    const markup = renderToStaticMarkup(
      <ShellSurfaceProvider value={{ visibility: "hidden" }}>
        <SurfaceProbe />
      </ShellSurfaceProvider>
    );

    assert.match(markup, /data-visibility="hidden"/);
    assert.match(markup, />hidden</);
  });
});
