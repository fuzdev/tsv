<script lang="ts">
	// A typed arrow param must not disable the last-arg hug: prettier keeps the
	// head args inline and breaks after `=>`, indenting the body.
	docManager.onEventName('documentClose', (document: Document) =>
		diagnosticsManager.removeDiagnostics(document)
	);

	// Return-type annotation on the arrow: still hugs.
	docManager.onEventName('documentClose', (document): DiagnosticsResult =>
		diagnosticsManager.removeDiagnostics(document)
	);

	// Conditional body: hugs, and the broken form drops the wrapping parens.
	docManager.onEventName('documentClose', (document: Document) =>
		cond ? removeDiagnosticsFor(document) : keepDiagnosticsFor(document)
	);

	// Object body: hugs, the object expands internally after `=>`.
	docManager.onEventName('documentClose', (document: Document) => ({
		identifier: document.identifier
	}));

	// Plain (non-member) callee with an object body: hugs the same way.
	registerDocumentHandler('documentClose', (document: Document) => ({
		identifier: document.identifier
	}));

	// Short case that fits on one line - stays inline.
	a.b('c', (d: D) => fn(d));
</script>
