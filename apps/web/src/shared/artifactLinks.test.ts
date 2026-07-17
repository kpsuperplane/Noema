import assert from "node:assert/strict";
import { test } from "node:test";
import {
  artifactDownloadHref,
  resolveArtifactReferenceLink,
  resolveTaskArtifactLink
} from "./artifactLinks";

const localDownload = "/artifacts/versions/artifact-version_1/download";
const externalUrl = "https://example.com/artifact";

test("artifact detail downloads remain available on web and are suppressed on desktop", () => {
  assert.equal(artifactDownloadHref(localDownload, { isDesktop: false }), localDownload);
  assert.equal(artifactDownloadHref(localDownload, { isDesktop: true }), null);
});

test("transcript references suppress desktop downloads while preserving external URLs", () => {
  assert.deepEqual(resolveArtifactReferenceLink(localDownload, externalUrl, { isDesktop: false }), {
    href: localDownload,
    external: false
  });
  assert.deepEqual(resolveArtifactReferenceLink(localDownload, externalUrl, { isDesktop: true }), {
    href: externalUrl,
    external: true
  });
  assert.deepEqual(resolveArtifactReferenceLink(null, externalUrl, { isDesktop: true }), {
    href: externalUrl,
    external: true
  });
});

test("task artifacts preserve external URLs but suppress local desktop routes", () => {
  assert.deepEqual(resolveTaskArtifactLink(localDownload, { isDesktop: false }), {
    href: localDownload,
    external: false
  });
  assert.equal(resolveTaskArtifactLink(localDownload, { isDesktop: true }), null);
  assert.deepEqual(resolveTaskArtifactLink(externalUrl, { isDesktop: true }), {
    href: externalUrl,
    external: true
  });
});

test("artifact link policy rejects unrelated local routes and unsafe URL schemes", () => {
  assert.equal(artifactDownloadHref("/settings", { isDesktop: false }), null);
  assert.equal(resolveTaskArtifactLink("javascript:alert(1)", { isDesktop: false }), null);
  assert.equal(
    resolveArtifactReferenceLink("https://attacker.invalid/not-a-download", null, {
      isDesktop: false
    }),
    null
  );
});
