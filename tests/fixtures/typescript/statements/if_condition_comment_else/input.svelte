<script lang="ts">
	// Short condition - fits on one line
	function a() {
		if (x && y) {
			a();
		}
		// comment
		else if (z) {
			b();
		}
	}

	// Long condition at deep indent - must break with proper indent
	class Foo {
		bar() {
			fn((node) => {
				if (
					ts.isExportDeclaration(noooode) &&
					noooode.exportClause &&
					ts.isNamedExports(noooode.exportClause)
				) {
					x();
				}
				// Check for default
				else if (ts.isExportAssignment(noooode)) {
					y();
				}
			});
		}
	}
</script>
