<script lang="ts">
	// Block body: empty statement dropped among real statements (comment preserved)
	function fn() {
		a();
		// keep me
		b();
	}

	// Bare block body: only empty statements collapses like a genuinely empty block
	{
	}

	// Static block: empty statement dropped among real statements
	class C {
		static {
			a();
		}
	}

	// Static block: only empty statements collapses like a genuinely empty static block
	class D {
		static {}
	}

	// Namespace body: empty statement dropped among real statements
	namespace N {
		a();
	}

	// Namespace body: only empty statements collapses like a genuinely empty namespace
	namespace M {}
</script>
