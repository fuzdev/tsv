<script lang="ts">
	// A line comment before a statement's `;` trails after it (the `;` binds to the
	// statement; emitting it before would swallow the `;`). A block stays before `;`.
	const a = 1; // c
	fn(); // d
	class C {
		e = 1; // f
	}
	// A same-line comment trails the `;`; an own-line comment drops to its own line
	// after the `;`.
	const g = 1;
	// own
	const h = 2;
</script>
