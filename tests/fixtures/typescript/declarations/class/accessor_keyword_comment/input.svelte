<script lang="ts">
	// Comment between accessor keyword and name is preserved
	class A {
		get /* c1 */ a() {
			return 1;
		}
		set /* c2 */ a(v) {}
	}

	// A run the author GLUED — a multi-line block ahead of a single-line one — and broke
	// after, in the accessor keyword→name gap: both formatters drop the name below the run
	class B {
		get /* c3
	d3 */ /* c4 */
		b() {
			return 1;
		}
	}
</script>
